//! How a body moves: the oscillator library and the straight pass, as plain data.
//!
//! Every motion is a closed form in time, so the forward model can ask where a body is at any
//! instant of a flight. `Linear` and `Circular` are the reference scripts' oscillators
//! (`traj_linear`; `traj_vertical_circle`, `traj_horizontal_circle` and `traj_inclined_circle` are
//! `Circular` with different `u` and `v`); `Pendulum` is their spherical pendulum with its slow
//! precession (`traj_pendulum` of `fig2_multi_directional.py`). `Sphere` is **not** from the
//! scripts or the paper: "forced motion on a sphere" (upgrade report §2.5.1) was not available to
//! read, and this is the slice's own parametrisation, to be checked with George.
//!
//! Frame: metres, x east, y north, z up; seconds; angles in degrees; frequencies in hertz.

use std::f64::consts::TAU;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where a body is as a function of time. Every position is `[x, y, z]` (east, north, up) in
/// metres in the world's frame; a displacement (`amplitude`, `u`, `v`) is in the same axes,
/// relative to its centre. Times are seconds on the scene's clock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Motion {
    /// At rest.
    Static {
        /// Where it is, metres.
        #[schemars(extend("x-unit" = "m"))]
        position: [f64; 3],
    },
    /// A straight pass at constant speed: at `from` until `start`, then towards `to`, where it stops.
    Pass {
        /// Where it waits before `start`, metres.
        #[schemars(extend("x-unit" = "m"))]
        from: [f64; 3],
        /// Where it stops, metres. Not the same point as `from`.
        #[schemars(extend("x-unit" = "m"))]
        to: [f64; 3],
        /// When it sets off, seconds.
        #[schemars(extend("x-unit" = "s"))]
        start: f64,
        /// Its speed on the way, metres per second.
        #[schemars(range(max = 1.0e4), extend("exclusiveMinimum" = 0.0))]
        #[schemars(extend("x-unit" = "m/s"))]
        speed: f64,
    },
    /// A linear oscillator: `centre + amplitude · cos(2π·frequency·t + phase)`.
    Linear {
        /// The middle of the swing, metres.
        #[schemars(extend("x-unit" = "m"))]
        centre: [f64; 3],
        /// The displacement at the top of the swing, metres: its direction is the line of motion.
        #[schemars(extend("x-unit" = "m"))]
        amplitude: [f64; 3],
        /// Hertz.
        #[schemars(range(min = 0.0, max = 1.0e3))]
        #[schemars(extend("x-unit" = "Hz"))]
        frequency: f64,
        /// Degrees.
        #[schemars(extend("x-unit" = "°"))]
        phase: f64,
    },
    /// A circle, or an ellipse: `centre + u · cos θ + v · sin θ`, with `θ = 2π·frequency·t + phase`.
    /// A circle when `u` and `v` are perpendicular and equally long.
    Circular {
        /// The middle of the orbit, metres.
        #[schemars(extend("x-unit" = "m"))]
        centre: [f64; 3],
        /// Where the body is at θ = 0, relative to the centre, metres.
        #[schemars(extend("x-unit" = "m"))]
        u: [f64; 3],
        /// Where it is a quarter of a turn later, relative to the centre, metres.
        #[schemars(extend("x-unit" = "m"))]
        v: [f64; 3],
        /// Turns per second, hertz.
        #[schemars(range(min = 0.0, max = 1.0e3))]
        #[schemars(extend("x-unit" = "Hz"))]
        frequency: f64,
        /// Degrees.
        #[schemars(extend("x-unit" = "°"))]
        phase: f64,
    },
    /// A spherical pendulum under gravity, small-amplitude solution with the leading precession:
    /// the bob traces an ellipse of semi-axes `major` and `minor` at `√(g/L)/2π` hertz, and the
    /// ellipse turns at `Ω = (3/8)·√(g/L)·major·minor/L²`. The bob hangs `√(L² − x² − y²)` below
    /// the pivot, so its height moves at twice the frequency of its swing.
    Pendulum {
        /// The point it hangs from, metres.
        #[schemars(extend("x-unit" = "m"))]
        pivot: [f64; 3],
        /// The length of the string, metres.
        #[schemars(range(max = 1.0e4), extend("exclusiveMinimum" = 0.0))]
        #[schemars(extend("x-unit" = "m"))]
        length: f64,
        /// The longer semi-axis of the bob's ellipse, metres; less than `length`.
        #[schemars(range(min = 0.0, max = 1.0e4))]
        #[schemars(extend("x-unit" = "m"))]
        major: f64,
        /// The shorter semi-axis, metres; zero for a plane pendulum.
        #[schemars(range(min = 0.0, max = 1.0e4))]
        #[schemars(extend("x-unit" = "m"))]
        minor: f64,
        /// The compass direction of the major axis at t = 0, degrees anticlockwise from east.
        #[schemars(extend("x-unit" = "°"))]
        azimuth: f64,
    },
    /// Forced motion on a sphere (this slice's own parametrisation; see the module's note): the
    /// body is `radius` from `centre`, at an angle `tilt · cos(2π·tilt_frequency·t)` from straight
    /// down, in a vertical plane that turns about the vertical at `spin_frequency`.
    Sphere {
        /// The centre of the sphere, metres.
        #[schemars(extend("x-unit" = "m"))]
        centre: [f64; 3],
        /// Metres.
        #[schemars(range(max = 1.0e4), extend("exclusiveMinimum" = 0.0))]
        #[schemars(extend("x-unit" = "m"))]
        radius: f64,
        /// The largest angle from straight down, degrees.
        #[schemars(range(min = 0.0, max = 180.0))]
        #[schemars(extend("x-unit" = "°"))]
        tilt: f64,
        /// Hertz.
        #[schemars(range(min = 0.0, max = 1.0e3))]
        #[schemars(extend("x-unit" = "Hz"))]
        tilt_frequency: f64,
        /// Turns of the plane per second, hertz.
        #[schemars(range(min = 0.0, max = 1.0e3))]
        #[schemars(extend("x-unit" = "Hz"))]
        spin_frequency: f64,
    },
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scaled(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

impl Motion {
    /// The names of the kinds of motion, as the spec writes them.
    pub const KINDS: [&'static str; 6] = ["Static", "Pass", "Linear", "Circular", "Pendulum", "Sphere"];

    /// A motion of the named kind that passes through or about `at` (`[x, y, z]`, metres, the
    /// world's frame) and is valid as it
    /// stands: what a body is given when its kind of motion is changed, to be edited from there.
    /// `None` for a name that is not a kind.
    #[must_use]
    pub fn starter(kind: &str, at: [f64; 3]) -> Option<Self> {
        let [x, y, z] = at;
        Some(match kind {
            "Static" => Self::Static { position: at },
            "Pass" => Self::Pass { from: at, to: [x + 4.0, y, z], start: 0.0, speed: 1.0 },
            "Linear" => Self::Linear { centre: at, amplitude: [0.0, 0.0, 0.5], frequency: 0.05, phase: 0.0 },
            "Circular" => Self::Circular {
                centre: at,
                u: [0.5, 0.0, 0.0],
                v: [0.0, 0.0, 0.5],
                frequency: 0.05,
                phase: 0.0,
            },
            "Pendulum" => {
                Self::Pendulum { pivot: [x, y, z + 2.0], length: 2.0, major: 0.5, minor: 0.0, azimuth: 0.0 }
            }
            "Sphere" => Self::Sphere {
                centre: [x, y, z + 1.0],
                radius: 1.0,
                tilt: 20.0,
                tilt_frequency: 0.05,
                spin_frequency: 0.01,
            },
            _ => return None,
        })
    }

    /// Where the body is at `t` seconds on the scene's clock: `[x, y, z]` (east, north, up),
    /// metres, in the world's frame. `gravity` is the site's, metres per second squared; only the
    /// pendulum uses it.
    #[must_use]
    pub fn position(&self, t: f64, gravity: f64) -> [f64; 3] {
        match *self {
            Self::Static { position } => position,
            Self::Pass { from, to, start, speed } => {
                let length = distance(from, to);
                let along = ((t - start) * speed).clamp(0.0, length);
                // Written so that the ends are exact: `from` before the start, `to` at the stop.
                if along <= 0.0 {
                    from
                } else if along >= length {
                    to
                } else {
                    add(from, scaled([to[0] - from[0], to[1] - from[1], to[2] - from[2]], along / length))
                }
            }
            Self::Linear { centre, amplitude, frequency, phase } => {
                add(centre, scaled(amplitude, (TAU * frequency * t + phase.to_radians()).cos()))
            }
            Self::Circular { centre, u, v, frequency, phase } => {
                let (sin, cos) = (TAU * frequency * t + phase.to_radians()).sin_cos();
                add(centre, add(scaled(u, cos), scaled(v, sin)))
            }
            Self::Pendulum { pivot, length, major, minor, azimuth } => {
                let swing = (gravity / length).sqrt();
                let precession = 0.375 * swing * major * minor / (length * length);
                let (x0, y0) = (major * (swing * t).cos(), minor * (swing * t).sin());
                let (sin, cos) = (precession * t + azimuth.to_radians()).sin_cos();
                let (x, y) = (x0 * cos - y0 * sin, x0 * sin + y0 * cos);
                add(pivot, [x, y, -(length * length - x * x - y * y).max(0.0).sqrt()])
            }
            Self::Sphere { centre, radius, tilt, tilt_frequency, spin_frequency } => {
                let from_down = tilt.to_radians() * (TAU * tilt_frequency * t).cos();
                let (sin, cos) = (TAU * spin_frequency * t).sin_cos();
                let across = radius * from_down.sin();
                add(centre, [across * cos, across * sin, -radius * from_down.cos()])
            }
        }
    }

    /// The times at which the motion is not smooth, seconds on the scene's clock, in rising
    /// order: a pass starts and stops. Empty for a motion that is smooth throughout.
    #[must_use]
    pub fn breaks(&self) -> Vec<f64> {
        match *self {
            Self::Pass { from, to, start, speed } => vec![start, start + distance(from, to) / speed],
            _ => Vec::new(),
        }
    }

    /// `(point, field)`: the point a drag moves (`[x, y, z]`, metres, the world's frame), and the
    /// path of that field within the motion (after `bodies[n].motion.`): the position of a static body, the start of a pass, the centre of an
    /// oscillator, the pivot of a pendulum. Moving it moves the whole path without changing its shape,
    /// except for a pass, whose far end stays where it is.
    #[must_use]
    pub const fn anchor(&self) -> ([f64; 3], &'static str) {
        match *self {
            Self::Static { position } => (position, "Static.position"),
            Self::Pass { from, .. } => (from, "Pass.from"),
            Self::Linear { centre, .. } => (centre, "Linear.centre"),
            Self::Circular { centre, .. } => (centre, "Circular.centre"),
            Self::Pendulum { pivot, .. } => (pivot, "Pendulum.pivot"),
            Self::Sphere { centre, .. } => (centre, "Sphere.centre"),
        }
    }

    /// The fundamental frequency of the motion, hertz, if it repeats: what a comb of harmonics is
    /// drawn from. `None` for a body at rest or on a pass, or an oscillator that does not move.
    #[must_use]
    pub fn frequency(&self, gravity: f64) -> Option<f64> {
        let frequency = match *self {
            Self::Static { .. } | Self::Pass { .. } => return None,
            Self::Linear { frequency, .. } | Self::Circular { frequency, .. } => frequency,
            Self::Pendulum { length, .. } => (gravity / length).sqrt() / TAU,
            Self::Sphere { tilt_frequency, .. } => tilt_frequency,
        };
        (frequency > 0.0).then_some(frequency)
    }

    /// One word for what kind of motion it is.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Static { .. } => "static",
            Self::Pass { .. } => "pass",
            Self::Linear { .. } => "linear",
            Self::Circular { .. } => "circular",
            Self::Pendulum { .. } => "pendulum",
            Self::Sphere { .. } => "sphere",
        }
    }

    /// Checks the numbers: finite, inside the spec's ranges, and describing a path.
    ///
    /// # Errors
    /// The first field that fails, by its path under `at` (such as `bodies[0].motion`).
    pub fn validate(&self, at: &str) -> Result<(), String> {
        use crate::spec::limits;
        let point = |name: &str, p: [f64; 3]| {
            p.iter().all(|c| c.is_finite() && c.abs() <= limits::POSITION).then_some(()).ok_or_else(|| {
                format!("{at}.{}.{name}: {p:?} must be within ±{} m", self.variant(), limits::POSITION)
            })
        };
        let number = |name: &str, value: f64, least: f64, most: f64, open: bool| {
            let inside =
                value.is_finite() && value <= most && if open { value > least } else { value >= least };
            inside.then_some(()).ok_or_else(|| {
                let bound = if open { "above" } else { "at least" };
                format!("{at}.{}.{name}: {value} must be {bound} {least} and at most {most}", self.variant())
            })
        };
        match *self {
            Self::Static { position } => point("position", position),
            Self::Pass { from, to, start, speed } => {
                point("from", from)?;
                point("to", to)?;
                number("start", start, -limits::DURATION, limits::DURATION, false)?;
                number("speed", speed, 0.0, limits::SPEED, true)?;
                if from == to {
                    return Err(format!("{at}.Pass.to: a pass needs somewhere to go; `to` is `from`"));
                }
                Ok(())
            }
            Self::Linear { centre, amplitude, frequency, phase } => {
                point("centre", centre)?;
                point("amplitude", amplitude)?;
                number("frequency", frequency, 0.0, limits::FREQUENCY, false)?;
                number("phase", phase, -limits::ANGLE, limits::ANGLE, false)
            }
            Self::Circular { centre, u, v, frequency, phase } => {
                point("centre", centre)?;
                point("u", u)?;
                point("v", v)?;
                number("frequency", frequency, 0.0, limits::FREQUENCY, false)?;
                number("phase", phase, -limits::ANGLE, limits::ANGLE, false)
            }
            Self::Pendulum { pivot, length, major, minor, azimuth } => {
                point("pivot", pivot)?;
                number("length", length, 0.0, limits::ROOM_SIZE, true)?;
                number("major", major, 0.0, limits::ROOM_SIZE, false)?;
                number("minor", minor, 0.0, limits::ROOM_SIZE, false)?;
                number("azimuth", azimuth, -limits::ANGLE, limits::ANGLE, false)?;
                if minor > major || major * major + minor * minor >= length * length {
                    return Err(format!(
                        "{at}.Pendulum.major: the ellipse ({major} m by {minor} m, major first) must fit inside \
                         the string's reach, {length} m"
                    ));
                }
                Ok(())
            }
            Self::Sphere { centre, radius, tilt, tilt_frequency, spin_frequency } => {
                point("centre", centre)?;
                number("radius", radius, 0.0, limits::ROOM_SIZE, true)?;
                number("tilt", tilt, 0.0, 180.0, false)?;
                number("tilt_frequency", tilt_frequency, 0.0, limits::FREQUENCY, false)?;
                number("spin_frequency", spin_frequency, 0.0, limits::FREQUENCY, false)
            }
        }
    }

    /// The variant's name as it is written in a scene file.
    const fn variant(&self) -> &'static str {
        match self {
            Self::Static { .. } => "Static",
            Self::Pass { .. } => "Pass",
            Self::Linear { .. } => "Linear",
            Self::Circular { .. } => "Circular",
            Self::Pendulum { .. } => "Pendulum",
            Self::Sphere { .. } => "Sphere",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: f64 = 9.81;

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        distance(a, b) < 1e-12
    }

    #[test]
    fn motions_follow_their_formulas() {
        let still = Motion::Static { position: [5.0, 0.0, 1.0] };
        assert_eq!(still.position(-3.0, G), still.position(1e5, G));

        // The scripts' linear oscillator: h = h0 + A cos(2π f t) at fixed D.
        let linear = Motion::Linear {
            centre: [5.0, 0.0, 1.0],
            amplitude: [0.0, 0.0, 1.0],
            frequency: 0.1,
            phase: 0.0,
        };
        assert!(close(linear.position(0.0, G), [5.0, 0.0, 2.0]));
        assert!(
            close(linear.position(2.5, G), [5.0, 0.0, 1.0]),
            "a quarter of a period later it is at the centre"
        );
        assert!(close(linear.position(5.0, G), [5.0, 0.0, 0.0]));
        assert!(close(linear.position(10.0, G), linear.position(0.0, G)), "it repeats after 1/f");
        // A phase of 90° is a quarter of a period early.
        let early = Motion::Linear {
            centre: [5.0, 0.0, 1.0],
            amplitude: [0.0, 0.0, 1.0],
            frequency: 0.1,
            phase: 90.0,
        };
        assert!(close(early.position(0.0, G), linear.position(2.5, G)));

        // The scripts' vertical circle: h = h0 + A sin φ, D = D0 + A cos φ.
        let circle = Motion::Circular {
            centre: [3.0, 0.0, 1.0],
            u: [0.5, 0.0, 0.0],
            v: [0.0, 0.0, 0.5],
            frequency: 0.25,
            phase: 0.0,
        };
        assert!(close(circle.position(0.0, G), [3.5, 0.0, 1.0]));
        assert!(close(circle.position(1.0, G), [3.0, 0.0, 1.5]));
        for step in 0..40 {
            let p = circle.position(0.1 * f64::from(step), G);
            assert!((distance(p, [3.0, 0.0, 1.0]) - 0.5).abs() < 1e-12, "always on the circle");
        }

        // Forced motion on a sphere stays on the sphere and passes straight below the centre.
        let sphere = Motion::Sphere {
            centre: [0.0, 0.0, 6.0],
            radius: 2.0,
            tilt: 30.0,
            tilt_frequency: 0.2,
            spin_frequency: 0.05,
        };
        for step in 0..60 {
            let p = sphere.position(0.37 * f64::from(step), G);
            assert!((distance(p, [0.0, 0.0, 6.0]) - 2.0).abs() < 1e-12);
        }
        assert!(
            close(sphere.position(1.25, G), [0.0, 0.0, 4.0]),
            "a quarter of a tilt period: straight down"
        );
        let top = sphere.position(0.0, G);
        assert!((top[0] - 1.0).abs() < 1e-12 && (top[2] - (6.0 - 3f64.sqrt())).abs() < 1e-12, "{top:?}");

        // Control: another frequency is another place.
        let faster = Motion::Linear {
            centre: [5.0, 0.0, 1.0],
            amplitude: [0.0, 0.0, 1.0],
            frequency: 0.2,
            phase: 0.0,
        };
        assert!(!close(faster.position(2.5, G), linear.position(2.5, G)));
    }

    #[test]
    fn the_pendulum_is_the_scripts_pendulum() {
        // fig2_multi_directional.py: L = 14.5 m, semi-axes 0.5 and 0.25 m; the source hangs at
        // about h0 = 1 m, at D = 3 m from the axis, the major axis along the line to the axis.
        let (length, major, minor) = (14.5, 0.5, 0.25);
        let pendulum =
            Motion::Pendulum { pivot: [3.0, 0.0, 1.0 + length], length, major, minor, azimuth: 0.0 };
        let swing = (G / length).sqrt();
        assert!((pendulum.frequency(G).unwrap() - 0.1309).abs() < 1e-4, "f0 = sqrt(g/L)/2π, about 0.1309 Hz");
        let precession = 0.375 * swing * major * minor / (length * length);
        for &t in &[0.0, 1.3, 77.7, 4321.0] {
            // The script's formulas, written out.
            let (x0, y0) = (major * (swing * t).cos(), minor * (swing * t).sin());
            let (x, y) = (
                x0 * (precession * t).cos() - y0 * (precession * t).sin(),
                x0 * (precession * t).sin() + y0 * (precession * t).cos(),
            );
            let h = 1.0 + length - (length * length - x * x - y * y).sqrt();
            assert!(close(pendulum.position(t, G), [3.0 + x, y, h]), "t = {t}");
        }
        // The bob is at its highest at both ends of a swing: its height runs at twice f0.
        let period = TAU / swing;
        let height = |t: f64| pendulum.position(t, G)[2];
        assert!(height(0.0) > height(0.25 * period) && height(0.5 * period) > height(0.25 * period));
        // Control: weaker gravity swings it more slowly.
        assert!(pendulum.frequency(1.62).unwrap() < 0.06);
    }

    #[test]
    fn a_pass_starts_and_stops_and_says_where() {
        let pass = Motion::Pass { from: [10.0, 0.0, -10.0], to: [10.0, 0.0, 15.0], start: 2.0, speed: 2.5 };
        assert_eq!(pass.position(-5.0, G), [10.0, 0.0, -10.0], "it waits at the start");
        assert_eq!(pass.position(2.0, G), [10.0, 0.0, -10.0]);
        assert!(close(pass.position(6.0, G), [10.0, 0.0, 0.0]), "4 s at 2.5 m/s is 10 m");
        assert_eq!(pass.position(12.0, G), [10.0, 0.0, 15.0], "25 m takes 10 s");
        assert_eq!(pass.position(99.0, G), [10.0, 0.0, 15.0], "and there it stays");
        // Its two kinks are what the forward model has to cut a flight at.
        assert_eq!(pass.breaks(), [2.0, 12.0]);
        assert!(pass.frequency(G).is_none());
        // Control: smooth motions have no breaks.
        let still = Motion::Static { position: [0.0; 3] };
        assert!(still.breaks().is_empty());
        let linear =
            Motion::Linear { centre: [0.0; 3], amplitude: [1.0, 0.0, 0.0], frequency: 1.0, phase: 0.0 };
        assert!(linear.breaks().is_empty());
        assert_eq!(linear.frequency(G), Some(1.0));
    }

    #[test]
    fn every_kind_has_a_starter_that_is_valid() {
        for kind in Motion::KINDS {
            let motion = Motion::starter(kind, [3.0, -1.0, 2.0]).unwrap();
            assert_eq!(motion.validate("motion"), Ok(()), "{kind}");
            // It is written as its name: the starter of `Pass` is a `Pass`.
            let written = serde_json::to_value(&motion).unwrap();
            assert_eq!(written.as_object().unwrap().keys().next().unwrap(), kind);
        }
        // Control: a name that is not a kind has none, and the list is the enum's own variants.
        assert_eq!(Motion::starter("Wobble", [0.0; 3]), None);
        let schema = serde_json::to_value(schemars::schema_for!(Motion)).unwrap();
        assert_eq!(schema["oneOf"].as_array().unwrap().len(), Motion::KINDS.len());
    }

    #[test]
    fn every_motion_names_the_point_a_drag_moves() {
        let motions = [
            Motion::Static { position: [1.0, 2.0, 3.0] },
            Motion::Pass { from: [1.0, 2.0, 3.0], to: [4.0, 2.0, 3.0], start: 0.0, speed: 1.0 },
            Motion::Linear {
                centre: [1.0, 2.0, 3.0],
                amplitude: [0.0, 0.0, 1.0],
                frequency: 0.1,
                phase: 0.0,
            },
            Motion::Circular {
                centre: [1.0, 2.0, 3.0],
                u: [1.0, 0.0, 0.0],
                v: [0.0, 1.0, 0.0],
                frequency: 0.1,
                phase: 0.0,
            },
            Motion::Pendulum { pivot: [1.0, 2.0, 3.0], length: 2.0, major: 0.5, minor: 0.0, azimuth: 0.0 },
            Motion::Sphere {
                centre: [1.0, 2.0, 3.0],
                radius: 1.0,
                tilt: 10.0,
                tilt_frequency: 0.1,
                spin_frequency: 0.0,
            },
        ];
        for motion in &motions {
            let (anchor, path) = motion.anchor();
            assert_eq!(anchor, [1.0, 2.0, 3.0], "{}", motion.kind());
            // The path is the field's place in the serialised motion: a drag sets it by that path.
            let json = serde_json::to_value(motion).unwrap();
            let (variant, field) = path.split_once('.').unwrap();
            assert_eq!(json[variant][field], serde_json::json!([1.0, 2.0, 3.0]), "{path}");
            assert_eq!(motion.validate("bodies[0].motion"), Ok(()), "{}", motion.kind());
        }
        // Control: the kinds are six different words.
        let mut kinds: Vec<&str> = motions.iter().map(Motion::kind).collect();
        kinds.dedup();
        assert_eq!(kinds.len(), 6);
    }

    #[test]
    fn motions_that_describe_no_path_are_refused_with_the_field() {
        let refused = [
            (Motion::Static { position: [f64::NAN, 0.0, 0.0] }, "Static.position"),
            (Motion::Static { position: [2e6, 0.0, 0.0] }, "Static.position"),
            (Motion::Pass { from: [0.0; 3], to: [0.0; 3], start: 0.0, speed: 1.0 }, "Pass.to"),
            (Motion::Pass { from: [0.0; 3], to: [1.0, 0.0, 0.0], start: 0.0, speed: 0.0 }, "Pass.speed"),
            (
                Motion::Linear { centre: [0.0; 3], amplitude: [1.0, 0.0, 0.0], frequency: -1.0, phase: 0.0 },
                "Linear.frequency",
            ),
            (
                Motion::Circular {
                    centre: [0.0; 3],
                    u: [1.0, 0.0, 0.0],
                    v: [0.0, f64::INFINITY, 0.0],
                    frequency: 1.0,
                    phase: 0.0,
                },
                "Circular.v",
            ),
            (
                Motion::Pendulum { pivot: [0.0; 3], length: 1.0, major: 2.0, minor: 0.0, azimuth: 0.0 },
                "Pendulum.major",
            ),
            (
                Motion::Pendulum { pivot: [0.0; 3], length: 0.0, major: 0.0, minor: 0.0, azimuth: 0.0 },
                "Pendulum.length",
            ),
            (
                Motion::Sphere {
                    centre: [0.0; 3],
                    radius: 1.0,
                    tilt: 200.0,
                    tilt_frequency: 1.0,
                    spin_frequency: 0.0,
                },
                "Sphere.tilt",
            ),
        ];
        for (motion, field) in refused {
            let why = motion.validate("bodies[2].motion").unwrap_err();
            assert!(why.starts_with(&format!("bodies[2].motion.{field}")), "`{why}` should name `{field}`");
        }
    }
}
