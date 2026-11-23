//! Small `f64` vectors and 4×4 matrices: enough for cameras and frames, no generics.
//!
//! Positions are metres in a right-handed frame with x east, y north and z up. Everything on the CPU
//! is `f64`; values are rounded to `f32` only after an offset from a nearby origin has been formed
//! (REBUILD §3.4).

use std::ops::{Add, Mul, Neg, Sub};

/// A vector or point, `f64`.
///
/// The type carries no unit and no frame: what it holds says which. Throughout the workspace a
/// position is metres in the world's frame (x east, y north, z up) unless its documentation says
/// otherwise (an offset from a tower's base, a camera's view space, normalised device
/// coordinates).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec3 {
    /// The first component: east, for a position or direction in the world's frame.
    pub x: f64,
    /// The second component: north, in the world's frame.
    pub y: f64,
    /// The third component: up, in the world's frame.
    pub z: f64,
}

impl Vec3 {
    /// The zero vector; as a position, the frame's origin.
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    /// The unit vector along x: east, in the world's frame.
    pub const X: Self = Self::new(1.0, 0.0, 0.0);
    /// The unit vector along y: north, in the world's frame.
    pub const Y: Self = Self::new(0.0, 1.0, 0.0);
    /// The unit vector along z: up, in the world's frame.
    pub const Z: Self = Self::new(0.0, 0.0, 1.0);

    /// The vector with components `x`, `y`, `z`, in whatever unit and frame the caller means.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// The vector `[x, y, z]`: the order every positional array in the scene spec uses.
    #[must_use]
    pub const fn from_array(a: [f64; 3]) -> Self {
        Self::new(a[0], a[1], a[2])
    }

    /// The components as `[x, y, z]`, unit and frame unchanged.
    #[must_use]
    pub const fn to_array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }

    /// `[x, y, z]` rounded to `f32`. Call it on offsets from a nearby origin, never on world
    /// positions: an `f32` holds a position 10 km from the origin to about a millimetre.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn to_f32(self) -> [f32; 3] {
        [self.x as f32, self.y as f32, self.z as f32]
    }

    /// The scalar product, in the product of the two vectors' units.
    #[must_use]
    pub fn dot(self, o: Self) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    /// The vector product `self × o`, right-handed: `X × Y = Z`.
    #[must_use]
    pub fn cross(self, o: Self) -> Self {
        Self::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }

    /// The Euclidean length, in the vector's own unit.
    #[must_use]
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// The unit vector along `self`; the zero vector stays zero.
    #[must_use]
    pub fn normalised(self) -> Self {
        let l = self.length();
        if l > 0.0 { self * (1.0 / l) } else { self }
    }
}

impl Add for Vec3 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}

/// A 4×4 matrix, `f64`, stored by columns (`m[column][row]`), acting on column vectors.
///
/// For cameras: world to view, view to clip, and their product. Not a general linear-algebra
/// type (there is no inverse), and not for the forward model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    /// The entries by columns: `m[column][row]`, so `m[3]` is the translation column
    /// `[x, y, z, w]` of an affine transform.
    pub m: [[f64; 4]; 4],
}

impl Mat4 {
    /// The matrix that changes nothing.
    pub const IDENTITY: Self =
        Self { m: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]] };

    /// A view matrix: a camera at `eye` looking at `target`, right-handed, looking down its own −z.
    /// `eye` and `target` are positions and `up` a direction (it need not be a unit vector, and
    /// must not be parallel to the line of sight), all in the world's frame, metres. The result
    /// takes world positions to view space: x right, y up, −z ahead, metres from the eye.
    #[must_use]
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let f = (target - eye).normalised();
        let s = f.cross(up).normalised();
        let u = s.cross(f);
        Self {
            m: [
                [s.x, u.x, -f.x, 0.0],
                [s.y, u.y, -f.y, 0.0],
                [s.z, u.z, -f.z, 0.0],
                [-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0],
            ],
        }
    }

    /// A perspective projection from view space to wgpu's clip space (depth 0 at `near`, 1 at
    /// `far`). `fov_y` is the full vertical angle in radians; `aspect` is width over height;
    /// `near` and `far` are positive distances ahead of the eye, metres.
    #[must_use]
    pub fn perspective(fov_y: f64, aspect: f64, near: f64, far: f64) -> Self {
        let f = 1.0 / (0.5 * fov_y).tan();
        let r = far / (near - far);
        Self {
            m: [
                [f / aspect, 0.0, 0.0, 0.0],
                [0.0, f, 0.0, 0.0],
                [0.0, 0.0, r, -1.0],
                [0.0, 0.0, r * near, 0.0],
            ],
        }
    }

    /// An orthographic projection from view space to wgpu's clip space. `half_height` is half the
    /// visible height in metres; `aspect` is width over height; `near` and `far` are distances
    /// ahead of the eye, metres (depth 0 at `near`, 1 at `far`).
    #[must_use]
    pub fn orthographic(half_height: f64, aspect: f64, near: f64, far: f64) -> Self {
        let r = 1.0 / (near - far);
        Self {
            m: [
                [1.0 / (half_height * aspect), 0.0, 0.0, 0.0],
                [0.0, 1.0 / half_height, 0.0, 0.0],
                [0.0, 0.0, r, 0.0],
                [0.0, 0.0, r * near, 1.0],
            ],
        }
    }

    /// The homogeneous image of a point: `self · (p, 1)`, as `[x, y, z, w]`, before any division
    /// by `w`.
    #[must_use]
    pub fn transform(&self, p: Vec3) -> [f64; 4] {
        let v = [p.x, p.y, p.z, 1.0];
        let mut out = [0.0; 4];
        for (row, o) in out.iter_mut().enumerate() {
            *o = (0..4).map(|col| self.m[col][row] * v[col]).sum();
        }
        out
    }

    /// A point taken to normalised device coordinates, or `None` when it is not in front of the
    /// camera. In the result `x` runs −1…1 from the left edge to the right, `y` −1…1 from the
    /// bottom to the top, and `z` is depth, 0 at the near plane and 1 at the far; values outside
    /// those ranges are off the screen. Meaningful when `self` is a projection times a view.
    #[must_use]
    pub fn project(&self, p: Vec3) -> Option<Vec3> {
        let [x, y, z, w] = self.transform(p);
        (w > 0.0).then(|| Vec3::new(x / w, y / w, z / w))
    }

    /// Rounded to `f32`, by columns (`[column][row]`), as a shader expects it. Round a matrix
    /// whose translation is small (a view relative to a nearby origin), not one that carries
    /// world positions.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn to_f32(&self) -> [[f32; 4]; 4] {
        self.m.map(|col| col.map(|v| v as f32))
    }
}

impl Mul for Mat4 {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        let mut m = [[0.0; 4]; 4];
        for (col, out_col) in m.iter_mut().enumerate() {
            for (row, out) in out_col.iter_mut().enumerate() {
                *out = (0..4).map(|k| self.m[k][row] * o.m[col][k]).sum();
            }
        }
        Self { m }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12
    }

    #[test]
    fn cross_product_is_right_handed() {
        assert_eq!(Vec3::X.cross(Vec3::Y), Vec3::Z);
        assert_eq!(Vec3::Y.cross(Vec3::Z), Vec3::X);
        // Control: the other order is the opposite vector, so a left-handed mistake would be seen.
        assert_eq!(Vec3::Y.cross(Vec3::X), -Vec3::Z);
    }

    #[test]
    fn look_at_puts_the_target_on_the_axis_ahead() {
        let eye = Vec3::new(3.0, -7.0, 2.0);
        let target = Vec3::new(-1.0, 4.0, 0.5);
        let view = Mat4::look_at(eye, target, Vec3::Z);
        let [x, y, z, w] = view.transform(target);
        assert!(close(x, 0.0) && close(y, 0.0) && close(w, 1.0));
        assert!(close(z, -(target - eye).length()), "the target is straight ahead, down -z: {z}");
        // "Up" stays up: a point above the target lands at positive y.
        assert!(view.transform(target + Vec3::Z)[1] > 0.0);
        // Control: a point to the side of the target is not on the axis.
        assert!(view.transform(target + Vec3::X)[0].abs() > 0.1);
    }

    #[test]
    fn perspective_maps_near_and_far_to_depth_zero_and_one() {
        let p = Mat4::perspective(1.0, 1.5, 0.1, 100.0);
        let near = p.project(Vec3::new(0.0, 0.0, -0.1)).unwrap();
        let far = p.project(Vec3::new(0.0, 0.0, -100.0)).unwrap();
        assert!(close(near.z, 0.0) && close(far.z, 1.0));
        // A point behind the camera does not project.
        assert!(p.project(Vec3::new(0.0, 0.0, 1.0)).is_none());
    }

    #[test]
    fn perspective_shrinks_with_distance_and_orthographic_does_not() {
        let persp = Mat4::perspective(0.8, 1.0, 0.1, 100.0);
        let ortho = Mat4::orthographic(5.0, 1.0, 0.1, 100.0);
        let near = Vec3::new(1.0, 1.0, -2.0);
        let far = Vec3::new(1.0, 1.0, -20.0);
        assert!(close(ortho.project(near).unwrap().x, ortho.project(far).unwrap().x));
        assert!(close(ortho.project(near).unwrap().x, 0.2));
        // Control arm: the same two points under perspective differ by their distance ratio.
        let ratio = persp.project(near).unwrap().x / persp.project(far).unwrap().x;
        assert!(close(ratio, 10.0), "ratio {ratio}");
    }

    #[test]
    fn matrix_product_applies_the_right_factor_first() {
        let view = Mat4::look_at(Vec3::new(0.0, -5.0, 0.0), Vec3::ZERO, Vec3::Z);
        let proj = Mat4::perspective(1.0, 1.0, 0.1, 50.0);
        let p = Vec3::new(0.3, 0.2, 0.1);
        let [vx, vy, vz, _] = view.transform(p);
        let two_steps = proj.transform(Vec3::new(vx, vy, vz));
        let one_step = (proj * view).transform(p);
        for i in 0..4 {
            assert!(close(two_steps[i], one_step[i]));
        }
        // Control: the product does not commute.
        assert!((view * proj).transform(p) != one_step);
    }

    cav_validate::case! {
        test: validation_perspective_scales_inversely_with_distance,
        name: "maths.perspective_divide",
        subsystem: "maths",
        slice: "S0",
        reference: ClosedForm,
        against: "the CPU matrix Mat4::perspective only, not the renderer: a point ten times further away projects ten times nearer the axis",
        unit: Relative,
        tolerance: 1e-12,
        measure: {
            let projection = Mat4::perspective(0.8, 1.0, 0.1, 100.0);
            let near = projection.project(Vec3::new(1.0, 1.0, -2.0)).unwrap().x;
            let far = projection.project(Vec3::new(1.0, 1.0, -20.0)).unwrap().x;
            cav_validate::Measured::Error((near / far / 10.0 - 1.0).abs())
        },
    }
}
