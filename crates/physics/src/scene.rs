//! The air in a room, stirred and heated by the bodies in it — one-way coupling.
//!
//! Bodies move as they move (prescribed, animated, or recorded from the rigid dynamics); the air
//! responds to them and does not push back. For air against solid bodies that is physically
//! excellent, not merely convenient: the density ratio is about 1000:1, so the back-reaction on a
//! body is a part in a thousand of its weight. The seam for two-way coupling exists
//! ([`crate::RigidBody::force`]) and is left unfilled.
//!
//! Each fluid step the bodies are deposited onto the fluid grid (cloud-in-cell): cells mostly
//! occupied become solid and carry the body's local velocity as a moving no-slip boundary, and a
//! body's surface exchanges heat with the air by its [`Heat`] condition. What is recorded for the
//! gravity read is the air's **mass anomaly** through the equation of state.

use world::{Channel, WorldSource};

use crate::fluid::{Air, Fluid, FluidParams, Heat};
use crate::grid::{Grid3, GridField};

/// The room: the fluid domain, the air's transport properties, and its equation of state.
#[derive(Clone, Copy, Debug)]
pub struct Room {
    pub grid: Grid3,
    pub params: FluidParams,
    pub air: Air,
}

impl Room {
    /// A room of `size` metres with its floor corner at `origin`, gridded at `h`, filled with air at
    /// `t_ref` kelvin, adiabatic walls.
    pub fn new(origin: [f64; 3], size: [f64; 3], h: f64, t_ref: f64) -> Self {
        let n = [
            (size[0] / h).round().max(1.0) as usize,
            (size[1] / h).round().max(1.0) as usize,
            (size[2] / h).round().max(1.0) as usize,
        ];
        Room {
            grid: Grid3::new(origin, h, n),
            params: FluidParams::air(t_ref),
            air: Air::default(),
        }
    }

    /// The film coefficient `h_c/(ρ c_p)` [m s⁻¹] for a surface heat-transfer coefficient `h_c`
    /// [W m⁻² K⁻¹] in this room's air.
    pub fn film(&self, t_surface: f64, h_c: f64) -> Heat {
        Heat::Film {
            t_surface,
            h_over_rho_cp: h_c / (self.air.density(self.params.t_ref) * self.air.c_p),
        }
    }
}

/// A body as the air sees it: where its mass is over time, how much room that mass takes up, and
/// how its surface exchanges heat.
pub struct Obstacle<'a> {
    pub source: &'a dyn WorldSource,
    /// Bulk density [kg m⁻³] — converts parcel mass to the volume it excludes air from.
    pub density: f64,
    pub heat: Heat,
}

/// Mark the cells the obstacles occupy at time `t` as solid, with their velocity and heat condition.
fn stamp(fluid: &mut Fluid, fixed: &[bool], obstacles: &[Obstacle<'_>], t: f64, dt: f64) {
    let grid = fluid.grid;
    let n = grid.len();
    let vol = grid.cell_volume();
    for (id, &is_fixed) in fixed.iter().enumerate() {
        fluid.solid[id] = is_fixed;
        if !is_fixed {
            fluid.solid_vel[id] = [0.0; 3];
            fluid.solid_heat[id] = Heat::Adiabatic;
        }
    }
    let (mut occ, mut mass) = (vec![0.0; n], vec![0.0; n]);
    let mut momentum = [vec![0.0; n], vec![0.0; n], vec![0.0; n]];
    for ob in obstacles {
        let (now, before) = (
            ob.source.world_cloud_at(t),
            ob.source.world_cloud_at(t - dt),
        );
        occ.iter_mut().for_each(|x| *x = 0.0);
        mass.iter_mut().for_each(|x| *x = 0.0);
        momentum
            .iter_mut()
            .for_each(|m| m.iter_mut().for_each(|x| *x = 0.0));
        let volumes: Vec<f64> = now.ms.iter().map(|m| m / ob.density).collect();
        // Mass that leaves the fluid grid is simply outside the room; the body itself is intact.
        grid.splat(&now, &volumes, &mut occ);
        grid.splat(&now, &now.ms, &mut mass);
        for (axis, (pos, prev)) in [
            (&now.xs, &before.xs),
            (&now.ys, &before.ys),
            (&now.zs, &before.zs),
        ]
        .into_iter()
        .enumerate()
        {
            let p: Vec<f64> = (0..now.len())
                .map(|i| now.ms[i] * (pos[i] - prev[i]) / dt)
                .collect();
            grid.splat(&now, &p, &mut momentum[axis]);
        }
        for id in 0..n {
            if occ[id] >= 0.5 * vol && !fixed[id] {
                fluid.solid[id] = true;
                fluid.solid_vel[id] = [
                    momentum[0][id] / mass[id],
                    momentum[1][id] / mass[id],
                    momentum[2][id] / mass[id],
                ];
                fluid.solid_heat[id] = ob.heat;
            }
        }
    }
}

/// Simulate the room's air from `t0` to `t1` around the obstacles, recording the mass anomaly every
/// `frame_dt`. Returns the fluid's final state and the recorded field (an ambient source).
///
/// `fixed` marks cells that are permanently solid (furniture, fittings) with their heat condition —
/// a radiator is a fixed cell with a film or isothermal surface. The step is the frame interval
/// subdivided to keep the Courant number at or below `cfl`.
pub fn simulate_air(
    room: &Room,
    fixed: &[(usize, Heat)],
    obstacles: &[Obstacle<'_>],
    t0: f64,
    t1: f64,
    frame_dt: f64,
    cfl: f64,
) -> (Fluid, GridField) {
    assert!(t1 > t0 && frame_dt > 0.0);
    let mut fluid = Fluid::new(room.grid, room.params);
    let mut is_fixed = vec![false; room.grid.len()];
    for &(id, heat) in fixed {
        is_fixed[id] = true;
        fluid.solid[id] = true;
        fluid.solid_heat[id] = heat;
    }
    let n_frames = ((t1 - t0) / frame_dt).ceil() as usize;
    let mut frames = Vec::with_capacity(n_frames + 1);
    stamp(&mut fluid, &is_fixed, obstacles, t0, frame_dt);
    frames.push(fluid.mass_anomaly(&room.air));
    for f in 0..n_frames {
        let t_frame = t0 + f as f64 * frame_dt;
        let sub = (frame_dt / fluid.cfl_dt(cfl, frame_dt)).ceil().max(1.0) as usize;
        let dt = frame_dt / sub as f64;
        for s in 0..sub {
            stamp(
                &mut fluid,
                &is_fixed,
                obstacles,
                t_frame + s as f64 * dt,
                dt,
            );
            fluid.step(dt);
        }
        stamp(&mut fluid, &is_fixed, obstacles, t_frame + frame_dt, dt);
        frames.push(fluid.mass_anomaly(&room.air));
    }
    let field = GridField::new(&room.grid, t0, frame_dt, frames, Channel::Ambient);
    (fluid, field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::Parcels;
    use gravity::FieldContribution;
    use instrument::{Detector, InstrumentConfig};
    use math::{Isometry3, Quat, Vec3};
    use shape::{voxelise, Cuboid, MassSpec, VoxelParams};

    fn crate_cloud() -> gravity::Cloud {
        voxelise(
            &Cuboid {
                half: [0.2, 0.2, 0.3],
            },
            &VoxelParams::pitch(0.1),
            MassSpec::Density(500.0),
        )
        .unwrap()
    }

    #[test]
    fn warm_body_sheds_a_plume_that_gravitates() {
        // A warm block standing on the floor of a still room. The air above it must rise, the warm
        // air must be lighter than ambient through the equation of state, and that anomaly must
        // produce a non-zero, time-varying phase in a nearby gradiometer.
        let room = Room::new([0.5, -1.0, 0.0], [2.0, 2.0, 2.4], 0.2, 293.15);
        let body = Parcels::rigid(crate_cloud(), |_| {
            Isometry3::new(Quat::identity(), Vec3::new(1.5, 0.0, 0.3))
        });
        let obstacles = [Obstacle {
            source: &body,
            density: 500.0,
            heat: room.film(310.0, 5.0),
        }];
        let (fluid, field) = simulate_air(&room, &[], &obstacles, 0.0, 12.0, 0.2, 0.8);

        // Rising air above the block.
        let above = room.grid.idx(5, 5, 5);
        let w_above = fluid.w[above];
        assert!(w_above > 0.02, "no plume: w = {w_above} m/s above the body");
        // Warm air is lighter: the total anomaly is negative and grows as heat goes in.
        let total = |f: &Vec<f64>| f.iter().sum::<f64>();
        let (early, late) = (
            total(&field.frames()[5]),
            total(field.frames().last().unwrap()),
        );
        assert!(
            late < early && early < 0.0,
            "mass anomaly {early:e} → {late:e} kg"
        );
        // Solid cells carry no air anomaly; the flow stays divergence-free.
        let inside = room.grid.idx(5, 5, 1);
        assert!(fluid.solid[inside] && field.frames().last().unwrap()[inside] == 0.0);
        let div = fluid
            .divergence()
            .iter()
            .fold(0.0f64, |m, d| m.max(d.abs()));
        assert!(div <= 1e-4, "divergence {div:e}");

        // The gradiometer sees it, and sees it change.
        let (cfg, det) = (InstrumentConfig::default(), Detector::new(0.0));
        let phase = |t: f64| {
            let [a, b] = field.per_ifo_fast(&cfg, &det, t).unwrap();
            b - a
        };
        let (p4, p12) = (phase(4.0), phase(12.0));
        assert!(
            p12.abs() > 0.0 && (p12 - p4).abs() > 0.1 * p12.abs(),
            "phase {p4:e} → {p12:e} rad"
        );
        eprintln!("plume: w = {w_above:.3} m/s, δm = {late:.3e} kg, ΔΦ(12 s) = {p12:.3e} rad");
    }

    #[test]
    fn static_thermal_field_has_a_gravity_signal() {
        // The Boussinesq guard, end to end: a temperature anomaly with no flow at all still reads
        // as a density anomaly. (Read the solver's working density instead and this is exactly zero.)
        let room = Room::new([1.0, -0.5, 0.5], [1.0, 1.0, 1.0], 0.25, 293.15);
        let mut fluid = Fluid::new(room.grid, room.params);
        let id = room.grid.idx(1, 2, 2);
        fluid.t[id] = 313.15;
        let frame = fluid.mass_anomaly(&room.air);
        let field = GridField::new(&room.grid, 0.0, 1.0, vec![frame; 4], Channel::Ambient);
        let v = field.potential(Vec3::new(0.0, 0.0, 1.0), 1.0);
        assert!(v > 0.0, "a mass deficit raises the potential: V = {v:e}");
    }

    #[test]
    fn moving_body_stirs_isothermal_air() {
        // No heat anywhere: a block pushed through the room sets the air moving (and only the air it
        // displaces), with no density anomaly — stirring alone does not gravitate.
        let room = Room::new([0.0, -1.0, 0.0], [3.0, 2.0, 2.0], 0.2, 293.15);
        let body = Parcels::rigid(crate_cloud(), |t| {
            Isometry3::new(Quat::identity(), Vec3::new(0.6 + 0.5 * t, 0.0, 0.9))
        });
        let obstacles = [Obstacle {
            source: &body,
            density: 500.0,
            heat: Heat::Adiabatic,
        }];
        let (fluid, field) = simulate_air(&room, &[], &obstacles, 0.0, 2.0, 0.2, 0.8);
        let speed = fluid.u.iter().fold(0.0f64, |m, x| m.max(x.abs()));
        assert!(
            speed > 0.2 && speed < 2.0,
            "air speed {speed} m/s for a 0.5 m/s body"
        );
        // Interpolating a uniform temperature leaves it uniform to rounding — a part in 10¹³ of a
        // cell's air mass, twelve orders below the thermal anomalies above.
        let worst = field
            .frames()
            .iter()
            .flatten()
            .fold(0.0f64, |m, x| m.max(x.abs()));
        let cell_air = room.air.density(293.15) * room.grid.cell_volume();
        assert!(
            worst <= 1e-12 * cell_air,
            "anomaly {worst:e} kg in isothermal air"
        );
    }
}
