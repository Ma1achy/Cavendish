//! Rigid-body dynamics: free flight, rotation, and impulse-based contact.
//!
//! Translation is velocity-Verlet (exact under constant acceleration). Rotation reuses the engine's
//! structure-preserving free-rigid-body splitting (`source::step`), so angular momentum is conserved
//! to rounding and energy does not drift. Contact is sequential impulses with restitution and
//! Coulomb friction, after the standard formulation (e.g. Szauer, *Game Physics Cookbook*, ch. 15),
//! with two changes that matter for a gravimetric data engine: penetration is corrected by moving
//! bodies apart rather than by a velocity bias (no energy is injected), and a body that has come to
//! rest is put to **sleep** — its pose is then exactly constant, so a settled object contributes no
//! jitter to the mass field.
//!
//! These dynamics set where the mass *is*; how accurately the gradiometer reads it is the forward
//! model's job, downstream. Supported contact pairs: sphere–plane, box–plane, sphere–sphere,
//! sphere–box. Box–box is not implemented and is refused loudly rather than passed through.

use gravity::Cloud;
use math::{Isometry3, Mat3, Quat, Vec3};
use source::{BodyMotion, SourceDynamics};

/// A collision shape in the body frame, centred on the centre of mass.
#[derive(Clone, Copy, Debug)]
pub enum Collider {
    Sphere { r: f64 },
    Box { half: [f64; 3] },
}

/// A static half-space: solid where `normal·x < offset`.
#[derive(Clone, Copy, Debug)]
pub struct Plane {
    pub normal: Vec3<f64>,
    pub offset: f64,
}

/// One rigid body.
pub struct RigidBody {
    /// Body-frame mass cloud, centre of mass at the origin.
    pub cloud: Cloud,
    pub collider: Collider,
    pub position: Vec3<f64>,
    /// World ← body.
    pub orientation: Quat,
    pub velocity: Vec3<f64>,
    /// World-frame angular velocity.
    pub angular_velocity: Vec3<f64>,
    pub restitution: f64,
    pub friction: f64,
    /// External force and torque (world frame) applied over the next step — the slot a fluid's
    /// back-reaction plugs into for two-way coupling. Zero unless something fills it.
    pub force: Vec3<f64>,
    pub torque: Vec3<f64>,
    mass: f64,
    /// Principal moments and the rotation body ← principal frame.
    moments: Vec3<f64>,
    principal: Quat,
    asleep: bool,
    quiet: f64,
}

fn zero() -> Vec3<f64> {
    Vec3::new(0.0, 0.0, 0.0)
}

/// The quaternion of a proper rotation matrix (columns = the rotated basis vectors).
fn quat_from_matrix(m: &Mat3<f64>) -> Quat {
    let a = &m.m;
    let trace = a[0][0] + a[1][1] + a[2][2];
    let q = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        Quat::new(
            0.25 * s,
            (a[2][1] - a[1][2]) / s,
            (a[0][2] - a[2][0]) / s,
            (a[1][0] - a[0][1]) / s,
        )
    } else if a[0][0] > a[1][1] && a[0][0] > a[2][2] {
        let s = (1.0 + a[0][0] - a[1][1] - a[2][2]).sqrt() * 2.0;
        Quat::new(
            (a[2][1] - a[1][2]) / s,
            0.25 * s,
            (a[0][1] + a[1][0]) / s,
            (a[0][2] + a[2][0]) / s,
        )
    } else if a[1][1] > a[2][2] {
        let s = (1.0 + a[1][1] - a[0][0] - a[2][2]).sqrt() * 2.0;
        Quat::new(
            (a[0][2] - a[2][0]) / s,
            (a[0][1] + a[1][0]) / s,
            0.25 * s,
            (a[1][2] + a[2][1]) / s,
        )
    } else {
        let s = (1.0 + a[2][2] - a[0][0] - a[1][1]).sqrt() * 2.0;
        Quat::new(
            (a[1][0] - a[0][1]) / s,
            (a[0][2] + a[2][0]) / s,
            (a[1][2] + a[2][1]) / s,
            0.25 * s,
        )
    };
    q.normalise()
}

impl RigidBody {
    /// A body at rest at `position`. The cloud is recentred on its centre of mass; its principal
    /// frame comes from its own second moment.
    pub fn new(cloud: Cloud, collider: Collider, position: Vec3<f64>) -> Self {
        let r = gravity::inertia(&cloud);
        let cloud = cloud.transformed(&Isometry3::new(Quat::identity(), -r.com));
        // Principal axes as a proper rotation (flip one axis if the eigenvector triad is left-handed).
        let mut axes = r.axes;
        let c = |j: usize| Vec3::new(axes.m[0][j], axes.m[1][j], axes.m[2][j]);
        if c(0).cross(c(1)).dot(c(2)) < 0.0 {
            for row in &mut axes.m {
                row[2] = -row[2];
            }
        }
        RigidBody {
            cloud,
            collider,
            position,
            orientation: Quat::identity(),
            velocity: zero(),
            angular_velocity: zero(),
            restitution: 0.3,
            friction: 0.5,
            force: zero(),
            torque: zero(),
            mass: r.mass,
            moments: Vec3::new(r.moments[0], r.moments[1], r.moments[2]),
            principal: quat_from_matrix(&axes),
            asleep: false,
            quiet: 0.0,
        }
    }

    pub fn mass(&self) -> f64 {
        self.mass
    }

    pub fn is_asleep(&self) -> bool {
        self.asleep
    }

    /// World ← principal frame.
    fn principal_to_world(&self) -> Quat {
        self.orientation * self.principal
    }

    /// `I⁻¹·v` in the world frame.
    fn inv_inertia(&self, v: Vec3<f64>) -> Vec3<f64> {
        let q = self.principal_to_world();
        let b = q.conjugate().rotate(v);
        q.rotate(Vec3::new(
            b.x / self.moments.x,
            b.y / self.moments.y,
            b.z / self.moments.z,
        ))
    }

    /// World-frame angular momentum `L = I·ω`.
    pub fn angular_momentum(&self) -> Vec3<f64> {
        let q = self.principal_to_world();
        let b = q.conjugate().rotate(self.angular_velocity);
        q.rotate(Vec3::new(
            b.x * self.moments.x,
            b.y * self.moments.y,
            b.z * self.moments.z,
        ))
    }

    /// Kinetic energy, translational plus rotational.
    pub fn kinetic_energy(&self) -> f64 {
        0.5 * self.mass * self.velocity.norm_squared()
            + 0.5 * self.angular_velocity.dot(self.angular_momentum())
    }

    fn pose(&self) -> Isometry3 {
        Isometry3::new(self.orientation, self.position)
    }

    fn point_velocity(&self, r: Vec3<f64>) -> Vec3<f64> {
        self.velocity + self.angular_velocity.cross(r)
    }

    fn apply_impulse(&mut self, j: Vec3<f64>, r: Vec3<f64>) {
        self.velocity = self.velocity + j.scale(1.0 / self.mass);
        self.angular_velocity = self.angular_velocity + self.inv_inertia(r.cross(j));
    }

    /// Free flight over `h` under `gravity` and the external force/torque slot.
    fn integrate(&mut self, gravity: Vec3<f64>, h: f64) {
        let a = gravity + self.force.scale(1.0 / self.mass);
        self.position = self.position + self.velocity.scale(h) + a.scale(0.5 * h * h);
        self.velocity = self.velocity + a.scale(h);
        self.angular_velocity = self.angular_velocity + self.inv_inertia(self.torque).scale(h);
        // Torque-free rotation in the principal frame, by the engine's momentum-splitting integrator.
        let q = self.principal_to_world();
        let mut omega = q.conjugate().rotate(self.angular_velocity);
        let mut qa = [q.w, q.x, q.y, q.z];
        source::step(&mut qa, &mut omega, self.moments, h);
        let q = Quat::new(qa[0], qa[1], qa[2], qa[3]);
        self.angular_velocity = q.rotate(omega);
        self.orientation = (q * self.principal.conjugate()).normalise();
    }
}

/// A contact: `normal` pushes body `a` away from `b` (or from the static plane when `b` is `None`).
struct Contact {
    a: usize,
    b: Option<usize>,
    normal: Vec3<f64>,
    point: Vec3<f64>,
    depth: f64,
}

/// The dynamic world: bodies, static planes, gravity, and a fixed internal step.
pub struct RigidWorld {
    pub bodies: Vec<RigidBody>,
    pub planes: Vec<Plane>,
    pub gravity: Vec3<f64>,
    /// The internal step [s].
    pub dt: f64,
    pub time: f64,
}

/// Below these speeds, for [`SLEEP_AFTER`] seconds while in contact, a body is put to sleep.
const SLEEP_LINEAR: f64 = 0.02;
const SLEEP_ANGULAR: f64 = 0.05;
const SLEEP_AFTER: f64 = 0.5;
/// Approach speeds below this do not bounce (kills the infinite micro-bounce at rest).
const REST_SPEED: f64 = 0.3;

impl RigidWorld {
    pub fn new(gravity: Vec3<f64>, dt: f64) -> Self {
        RigidWorld {
            bodies: Vec::new(),
            planes: Vec::new(),
            gravity,
            dt,
            time: 0.0,
        }
    }

    /// A floor at height `z`.
    pub fn with_floor(mut self, z: f64) -> Self {
        self.planes.push(Plane {
            normal: Vec3::new(0.0, 0.0, 1.0),
            offset: z,
        });
        self
    }

    pub fn add(&mut self, body: RigidBody) -> usize {
        self.bodies.push(body);
        self.bodies.len() - 1
    }

    fn box_corners(body: &RigidBody, half: [f64; 3]) -> [Vec3<f64>; 8] {
        let pose = body.pose();
        let mut out = [zero(); 8];
        for (n, o) in out.iter_mut().enumerate() {
            let s = |bit: usize, h: f64| if n >> bit & 1 == 0 { -h } else { h };
            *o = pose.apply(Vec3::new(s(0, half[0]), s(1, half[1]), s(2, half[2])));
        }
        out
    }

    fn contacts(&self) -> Vec<Contact> {
        let mut out = Vec::new();
        for (a, body) in self.bodies.iter().enumerate() {
            for plane in &self.planes {
                match body.collider {
                    Collider::Sphere { r } => {
                        let depth = r - (plane.normal.dot(body.position) - plane.offset);
                        if depth > 0.0 {
                            out.push(Contact {
                                a,
                                b: None,
                                normal: plane.normal,
                                point: body.position - plane.normal.scale(r),
                                depth,
                            });
                        }
                    }
                    Collider::Box { half } => {
                        for corner in Self::box_corners(body, half) {
                            let depth = plane.offset - plane.normal.dot(corner);
                            if depth > 0.0 {
                                out.push(Contact {
                                    a,
                                    b: None,
                                    normal: plane.normal,
                                    point: corner,
                                    depth,
                                });
                            }
                        }
                    }
                }
            }
            for (b, other) in self.bodies.iter().enumerate().skip(a + 1) {
                match (body.collider, other.collider) {
                    (Collider::Sphere { r: ra }, Collider::Sphere { r: rb }) => {
                        let d = body.position - other.position;
                        let dist = d.norm();
                        if dist < ra + rb && dist > 0.0 {
                            let normal = d.scale(1.0 / dist);
                            out.push(Contact {
                                a,
                                b: Some(b),
                                normal,
                                point: other.position + normal.scale(rb - 0.5 * (ra + rb - dist)),
                                depth: ra + rb - dist,
                            });
                        }
                    }
                    (Collider::Sphere { r }, Collider::Box { half }) => {
                        if let Some(c) = sphere_box(body.position, r, other, half) {
                            out.push(Contact { a, b: Some(b), ..c });
                        }
                    }
                    (Collider::Box { half }, Collider::Sphere { r }) => {
                        if let Some(c) = sphere_box(other.position, r, body, half) {
                            // Found with the sphere as `a`; here the box is `a`, so flip the normal.
                            out.push(Contact {
                                a,
                                b: Some(b),
                                normal: -c.normal,
                                ..c
                            });
                        }
                    }
                    (Collider::Box { half: ha }, Collider::Box { half: hb }) => {
                        let reach = |h: [f64; 3]| (h[0] * h[0] + h[1] * h[1] + h[2] * h[2]).sqrt();
                        assert!(
                            (body.position - other.position).norm() > reach(ha) + reach(hb),
                            "box–box contact is not implemented: bodies {a} and {b} are close enough to touch"
                        );
                    }
                }
            }
        }
        out
    }

    /// Advance the world by one internal step.
    pub fn step(&mut self) {
        let h = self.dt;
        for body in self.bodies.iter_mut().filter(|b| !b.asleep) {
            body.integrate(self.gravity, h);
        }
        let contacts = self.contacts();

        // Velocity solve: sequential impulses, the normal impulse accumulated and clamped ≥ 0.
        let n = contacts.len();
        let mut target = vec![0.0; n];
        for (c, tgt) in contacts.iter().zip(target.iter_mut()) {
            let vn = self.relative_velocity(c).dot(c.normal);
            let e = match c.b {
                Some(b) => self.bodies[c.a].restitution.min(self.bodies[b].restitution),
                None => self.bodies[c.a].restitution,
            };
            *tgt = if vn < -REST_SPEED { -e * vn } else { 0.0 };
            // A real impact wakes whatever it hits.
            if vn < -REST_SPEED {
                self.bodies[c.a].asleep = false;
                if let Some(b) = c.b {
                    self.bodies[b].asleep = false;
                }
            }
        }
        let mut normal_sum = vec![0.0; n];
        for _ in 0..20 {
            for (i, c) in contacts.iter().enumerate() {
                let (ra, rb) = self.arms(c);
                let rel = self.relative_velocity(c);
                let vn = rel.dot(c.normal);
                let kn = self.effective_mass(c, c.normal, ra, rb);
                let new_sum = (normal_sum[i] + (target[i] - vn) / kn).max(0.0);
                let jn = new_sum - normal_sum[i];
                normal_sum[i] = new_sum;
                self.apply(c, c.normal.scale(jn), ra, rb);

                // Coulomb friction along the sliding direction, bounded by μ × the normal impulse.
                let rel = self.relative_velocity(c);
                let tangent = rel - c.normal.scale(rel.dot(c.normal));
                let speed = tangent.norm();
                if speed > 1e-12 {
                    let t = tangent.scale(1.0 / speed);
                    let mu = match c.b {
                        Some(b) => (self.bodies[c.a].friction * self.bodies[b].friction).sqrt(),
                        None => self.bodies[c.a].friction,
                    };
                    let kt = self.effective_mass(c, t, ra, rb);
                    let jt = (speed / kt).min(mu * normal_sum[i]);
                    self.apply(c, t.scale(-jt), ra, rb);
                }
            }
        }

        // Position correction: move the bodies apart along the normal, shared by inverse mass.
        for c in &contacts {
            let push =
                0.8 * c.depth / contacts.iter().filter(|o| o.a == c.a && o.b == c.b).count() as f64;
            let wa = if self.bodies[c.a].asleep {
                0.0
            } else {
                1.0 / self.bodies[c.a].mass
            };
            let wb = c.b.map_or(0.0, |b| {
                if self.bodies[b].asleep {
                    0.0
                } else {
                    1.0 / self.bodies[b].mass
                }
            });
            if wa + wb == 0.0 {
                continue;
            }
            let a = &mut self.bodies[c.a];
            a.position = a.position + c.normal.scale(push * wa / (wa + wb));
            if let Some(b) = c.b {
                let b = &mut self.bodies[b];
                b.position = b.position - c.normal.scale(push * wb / (wa + wb));
            }
        }

        // Sleep: a body in contact that has been slow for long enough stops exactly.
        let touching: Vec<bool> = (0..self.bodies.len())
            .map(|i| contacts.iter().any(|c| c.a == i || c.b == Some(i)))
            .collect();
        for (body, &touch) in self.bodies.iter_mut().zip(&touching) {
            let slow =
                body.velocity.norm() < SLEEP_LINEAR && body.angular_velocity.norm() < SLEEP_ANGULAR;
            body.quiet = if touch && slow { body.quiet + h } else { 0.0 };
            if body.quiet >= SLEEP_AFTER {
                body.asleep = true;
                body.velocity = zero();
                body.angular_velocity = zero();
            }
        }
        self.time += h;
    }

    fn arms(&self, c: &Contact) -> (Vec3<f64>, Vec3<f64>) {
        let ra = c.point - self.bodies[c.a].position;
        let rb = c.b.map_or(zero(), |b| c.point - self.bodies[b].position);
        (ra, rb)
    }

    fn relative_velocity(&self, c: &Contact) -> Vec3<f64> {
        let (ra, rb) = self.arms(c);
        let va = self.bodies[c.a].point_velocity(ra);
        let vb = c.b.map_or(zero(), |b| self.bodies[b].point_velocity(rb));
        va - vb
    }

    /// The inverse effective mass of the contact along direction `d`. A sleeping body is immovable.
    fn effective_mass(&self, c: &Contact, d: Vec3<f64>, ra: Vec3<f64>, rb: Vec3<f64>) -> f64 {
        let term = |body: &RigidBody, r: Vec3<f64>| {
            if body.asleep {
                0.0
            } else {
                1.0 / body.mass + d.dot(body.inv_inertia(r.cross(d)).cross(r))
            }
        };
        let k = term(&self.bodies[c.a], ra) + c.b.map_or(0.0, |b| term(&self.bodies[b], rb));
        if k > 0.0 {
            k
        } else {
            f64::INFINITY
        }
    }

    fn apply(&mut self, c: &Contact, j: Vec3<f64>, ra: Vec3<f64>, rb: Vec3<f64>) {
        if !self.bodies[c.a].asleep {
            self.bodies[c.a].apply_impulse(j, ra);
        }
        if let Some(b) = c.b {
            if !self.bodies[b].asleep {
                self.bodies[b].apply_impulse(-j, rb);
            }
        }
    }

    /// Run from the current time to `t_end`, sampling every body every `frame_dt`, and return each
    /// body's recorded motion as a rigid source the forward model can read.
    pub fn record(mut self, t_end: f64, frame_dt: f64) -> Vec<RecordedRigid> {
        let t0 = self.time;
        let per_frame = (frame_dt / self.dt).round().max(1.0) as usize;
        let frame_dt = per_frame as f64 * self.dt;
        let frames = ((t_end - t0) / frame_dt).ceil() as usize;
        let sample = |world: &RigidWorld, tracks: &mut Vec<Vec<Sample>>| {
            for (track, body) in tracks.iter_mut().zip(&world.bodies) {
                track.push(Sample {
                    position: body.position,
                    orientation: body.orientation,
                    velocity: body.velocity,
                    angular_velocity: body.angular_velocity,
                });
            }
        };
        let mut tracks = vec![Vec::with_capacity(frames + 1); self.bodies.len()];
        sample(&self, &mut tracks);
        for _ in 0..frames {
            for _ in 0..per_frame {
                self.step();
            }
            sample(&self, &mut tracks);
        }
        self.bodies
            .into_iter()
            .zip(tracks)
            .map(|(body, samples)| RecordedRigid {
                cloud: body.cloud,
                t0,
                dt: frame_dt,
                samples,
            })
            .collect()
    }
}

/// Sphere against an oriented box: the contact with the sphere as body `a` (normal pushes the sphere
/// away from the box). Indices are filled in by the caller.
fn sphere_box(centre: Vec3<f64>, r: f64, boxed: &RigidBody, half: [f64; 3]) -> Option<Contact> {
    let pose = boxed.pose();
    let local = pose.inverse().apply(centre);
    let l = [local.x, local.y, local.z];
    let clamped = [
        l[0].clamp(-half[0], half[0]),
        l[1].clamp(-half[1], half[1]),
        l[2].clamp(-half[2], half[2]),
    ];
    let closest = Vec3::new(clamped[0], clamped[1], clamped[2]);
    let d = local - closest;
    let dist = d.norm();
    let (normal_local, depth, point_local) = if dist > 0.0 {
        if dist >= r {
            return None;
        }
        (d.scale(1.0 / dist), r - dist, closest)
    } else {
        // Centre inside the box: push out through the nearest face.
        let (axis, _) = (0..3)
            .map(|a| (a, half[a] - l[a].abs()))
            .min_by(|x, y| x.1.total_cmp(&y.1))
            .unwrap();
        let mut n = [0.0; 3];
        n[axis] = l[axis].signum();
        let mut p = l;
        p[axis] = n[axis] * half[axis];
        (
            Vec3::new(n[0], n[1], n[2]),
            r + half[axis] - l[axis].abs(),
            Vec3::new(p[0], p[1], p[2]),
        )
    };
    Some(Contact {
        a: 0,
        b: None,
        normal: pose.rotation.rotate(normal_local),
        point: pose.apply(point_local),
        depth,
    })
}

#[derive(Clone, Copy)]
struct Sample {
    position: Vec3<f64>,
    orientation: Quat,
    velocity: Vec3<f64>,
    angular_velocity: Vec3<f64>,
}

/// A simulated body's motion, replayed: a fixed body cloud carried by the recorded pose. This is a
/// plain rigid source, so the forward model evaluates it on the amortised (body-frame) path.
///
/// Between samples the position is a cubic Hermite through the recorded positions and velocities
/// (exact for ballistic flight) and the orientation is the normalised linear blend of the two
/// recorded quaternions.
pub struct RecordedRigid {
    cloud: Cloud,
    t0: f64,
    dt: f64,
    samples: Vec<Sample>,
}

impl RecordedRigid {
    /// The time span the recording covers.
    pub fn span(&self) -> (f64, f64) {
        (self.t0, self.t0 + (self.samples.len() - 1) as f64 * self.dt)
    }

    fn bracket(&self, t: f64) -> (usize, f64) {
        let s = (t - self.t0) / self.dt;
        let last = (self.samples.len() - 1) as f64;
        assert!(
            s >= -1e-9 && s <= last + 1e-9,
            "recorded body queried at t = {t} outside its span {:?}",
            self.span()
        );
        let k = (s.floor().max(0.0) as usize).min(self.samples.len() - 2);
        (k, (s - k as f64).clamp(0.0, 1.0))
    }
}

impl SourceDynamics for RecordedRigid {
    fn body_cloud(&self) -> &Cloud {
        &self.cloud
    }

    fn pose_at(&self, t: f64) -> Isometry3 {
        let (k, u) = self.bracket(t);
        let (a, b) = (&self.samples[k], &self.samples[k + 1]);
        let (u2, u3) = (u * u, u * u * u);
        let position = a.position.scale(2.0 * u3 - 3.0 * u2 + 1.0)
            + a.velocity.scale((u3 - 2.0 * u2 + u) * self.dt)
            + b.position.scale(-2.0 * u3 + 3.0 * u2)
            + b.velocity.scale((u3 - u2) * self.dt);
        let (qa, mut qb) = (a.orientation, b.orientation);
        if qa.w * qb.w + qa.x * qb.x + qa.y * qb.y + qa.z * qb.z < 0.0 {
            qb = Quat::new(-qb.w, -qb.x, -qb.y, -qb.z);
        }
        let mix = |x: f64, y: f64| (1.0 - u) * x + u * y;
        let rotation = Quat::new(
            mix(qa.w, qb.w),
            mix(qa.x, qb.x),
            mix(qa.y, qb.y),
            mix(qa.z, qb.z),
        )
        .normalise();
        Isometry3::new(rotation, position)
    }

    fn motion_at(&self, t: f64) -> BodyMotion {
        let (k, u) = self.bracket(t);
        let (a, b) = (&self.samples[k], &self.samples[k + 1]);
        let mix = |x: Vec3<f64>, y: Vec3<f64>| x.scale(1.0 - u) + y.scale(u);
        let to_body = self.pose_at(t).rotation.conjugate();
        BodyMotion {
            velocity: mix(a.velocity, b.velocity),
            acceleration: (b.velocity - a.velocity).scale(1.0 / self.dt),
            angular_velocity: to_body.rotate(mix(a.angular_velocity, b.angular_velocity)),
            angular_acceleration: to_body
                .rotate((b.angular_velocity - a.angular_velocity).scale(1.0 / self.dt)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shape::{voxelise, Cuboid, MassSpec, Sphere, VoxelParams};

    fn ball(r: f64, mass: f64) -> Cloud {
        voxelise(
            &Sphere { r },
            &VoxelParams::pitch(r / 3.0),
            MassSpec::Total(mass),
        )
        .unwrap()
    }

    fn brick(half: [f64; 3], mass: f64) -> Cloud {
        voxelise(
            &Cuboid { half },
            &VoxelParams::pitch(half[0] / 2.0),
            MassSpec::Total(mass),
        )
        .unwrap()
    }

    const G: f64 = 9.81;

    fn down() -> Vec3<f64> {
        Vec3::new(0.0, 0.0, -G)
    }

    #[test]
    fn free_fall_is_ballistic() {
        // Velocity-Verlet is exact under constant acceleration: position matches ½gt² to rounding.
        let mut world = RigidWorld::new(down(), 1e-3);
        let mut b = RigidBody::new(
            ball(0.1, 2.0),
            Collider::Sphere { r: 0.1 },
            Vec3::new(0.0, 0.0, 50.0),
        );
        b.velocity = Vec3::new(1.0, -2.0, 3.0);
        world.add(b);
        for _ in 0..2000 {
            world.step();
        }
        let t = 2.0;
        let want = Vec3::new(1.0 * t, -2.0 * t, 50.0 + 3.0 * t - 0.5 * G * t * t);
        assert!(
            (world.bodies[0].position - want).norm() <= 1e-10,
            "{:?} vs {want:?}",
            world.bodies[0].position
        );
    }

    #[test]
    fn free_rotation_conserves_angular_momentum_and_energy() {
        // An asymmetric brick tumbling about its unstable axis for 20 s: |L| to rounding, E bounded.
        let mut world = RigidWorld::new(zero(), 1e-3);
        let mut b = RigidBody::new(
            brick([0.1, 0.2, 0.4], 5.0),
            Collider::Sphere { r: 0.01 },
            zero(),
        );
        b.angular_velocity = Vec3::new(0.05, 3.0, 0.02);
        world.add(b);
        let (l0, e0) = (
            world.bodies[0].angular_momentum(),
            world.bodies[0].kinetic_energy(),
        );
        let mut worst_e = 0.0f64;
        for _ in 0..20_000 {
            world.step();
            worst_e = worst_e.max((world.bodies[0].kinetic_energy() - e0).abs() / e0);
        }
        let l1 = world.bodies[0].angular_momentum();
        assert!(
            (l1 - l0).norm() / l0.norm() <= 1e-10,
            "L drift {:e}",
            (l1 - l0).norm() / l0.norm()
        );
        assert!(worst_e <= 1e-5, "energy excursion {worst_e:e}");
    }

    #[test]
    fn bounce_height_follows_restitution() {
        // Dropped from 1 m with e = 0.6: the first rebound peaks at e²·1 m.
        let mut world = RigidWorld::new(down(), 2e-4).with_floor(0.0);
        let r = 0.1;
        let mut b = RigidBody::new(
            ball(r, 1.0),
            Collider::Sphere { r },
            Vec3::new(0.0, 0.0, 1.0 + r),
        );
        b.restitution = 0.6;
        world.add(b);
        let (mut bounced, mut peak) = (false, 0.0f64);
        for _ in 0..10_000 {
            world.step();
            let body = &world.bodies[0];
            if body.velocity.z > 0.0 {
                bounced = true;
            }
            if bounced {
                peak = peak.max(body.position.z - r);
                if body.velocity.z < 0.0 {
                    break;
                }
            }
        }
        assert!((peak - 0.36).abs() <= 0.01, "rebound height {peak}");
    }

    #[test]
    fn equal_spheres_exchange_velocity_and_conserve_momentum() {
        let mut world = RigidWorld::new(zero(), 1e-4);
        let mk = |x: f64, v: f64| {
            let mut b = RigidBody::new(
                ball(0.1, 3.0),
                Collider::Sphere { r: 0.1 },
                Vec3::new(x, 0.0, 0.0),
            );
            b.velocity = Vec3::new(v, 0.0, 0.0);
            b.restitution = 1.0;
            b.friction = 0.0;
            b
        };
        world.add(mk(-0.5, 2.0));
        world.add(mk(0.5, 0.0));
        for _ in 0..10_000 {
            world.step();
        }
        let (a, b) = (&world.bodies[0], &world.bodies[1]);
        assert!(
            (a.velocity.x * 3.0 + b.velocity.x * 3.0 - 6.0).abs() <= 1e-10,
            "momentum"
        );
        assert!(
            a.velocity.x.abs() <= 1e-9 && (b.velocity.x - 2.0).abs() <= 1e-9,
            "{} {}",
            a.velocity.x,
            b.velocity.x
        );
    }

    #[test]
    #[allow(clippy::float_cmp)] // a sleeping body's pose is exactly constant
    fn dropped_box_settles_and_sleeps_exactly() {
        // A tilted box dropped on the floor comes to rest flat, goes to sleep, and from then on its
        // recorded pose is constant bit for bit — no resting jitter in the mass field.
        let half = [0.2, 0.15, 0.1];
        let mut world = RigidWorld::new(down(), 5e-4).with_floor(0.0);
        let mut b = RigidBody::new(
            brick(half, 10.0),
            Collider::Box { half },
            Vec3::new(0.0, 0.0, 0.5),
        );
        b.orientation = Quat::from_axis_angle(Vec3::new(1.0, 0.3, 0.0), 0.4);
        world.add(b);
        let tracks = world.record(6.0, 0.02);
        let track = &tracks[0];
        let (a, b) = (track.pose_at(5.0), track.pose_at(5.9));
        assert_eq!(a.translation.z, b.translation.z);
        assert_eq!((a.rotation.w, a.rotation.x), (b.rotation.w, b.rotation.x));
        assert_eq!(track.motion_at(5.5).velocity.norm(), 0.0);
        // Resting on one of its faces: the centre is at one of the half-extents above the floor.
        let z = a.translation.z;
        assert!(
            half.iter().any(|h| (z - h).abs() <= 2e-3),
            "rest height {z}"
        );
        assert!((track.body_cloud().total_mass() - 10.0).abs() <= 1e-12);
    }

    #[test]
    fn sphere_rests_on_a_box() {
        let half = [0.3, 0.3, 0.1];
        let mut world = RigidWorld::new(down(), 5e-4).with_floor(0.0);
        world.add(RigidBody::new(
            brick(half, 50.0),
            Collider::Box { half },
            Vec3::new(0.0, 0.0, 0.1),
        ));
        let mut s = RigidBody::new(
            ball(0.1, 1.0),
            Collider::Sphere { r: 0.1 },
            Vec3::new(0.02, 0.0, 0.5),
        );
        s.restitution = 0.0;
        world.add(s);
        for _ in 0..8000 {
            world.step();
        }
        let z = world.bodies[1].position.z;
        assert!(
            (z - 0.3).abs() <= 5e-3,
            "sphere should sit on top of the box at z = 0.3, got {z}"
        );
    }

    #[test]
    fn recording_reproduces_ballistic_flight_between_samples() {
        // Hermite interpolation of a coarse recording recovers the parabola between frames.
        let mut world = RigidWorld::new(down(), 1e-3);
        let mut b = RigidBody::new(
            ball(0.1, 2.0),
            Collider::Sphere { r: 0.1 },
            Vec3::new(0.0, 0.0, 20.0),
        );
        b.velocity = Vec3::new(0.5, 0.0, 4.0);
        world.add(b);
        let track = &world.record(1.5, 0.1)[0];
        for &t in &[0.033, 0.75, 1.234] {
            let want = Vec3::new(0.5 * t, 0.0, 20.0 + 4.0 * t - 0.5 * G * t * t);
            assert!((track.pose_at(t).translation - want).norm() <= 1e-10);
            assert!((track.motion_at(t).velocity.z - (4.0 - G * t)).abs() <= 1e-10);
        }
    }

    #[test]
    #[should_panic(expected = "box–box contact is not implemented")]
    fn box_box_contact_is_refused() {
        let half = [0.2; 3];
        let mut world = RigidWorld::new(zero(), 1e-3);
        world.add(RigidBody::new(
            brick(half, 1.0),
            Collider::Box { half },
            zero(),
        ));
        world.add(RigidBody::new(
            brick(half, 1.0),
            Collider::Box { half },
            Vec3::new(0.3, 0.0, 0.0),
        ));
        world.step();
    }
}
