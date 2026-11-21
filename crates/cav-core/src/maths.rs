//! Small `f64` vectors and 4×4 matrices: enough for cameras and frames, no generics.
//!
//! Positions are metres in a right-handed frame with x east, y north and z up. Everything on the CPU
//! is `f64`; values are rounded to `f32` only after an offset from a nearby origin has been formed
//! (REBUILD §3.4).

use std::ops::{Add, Mul, Neg, Sub};

/// A vector or point, `f64`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const X: Self = Self::new(1.0, 0.0, 0.0);
    pub const Y: Self = Self::new(0.0, 1.0, 0.0);
    pub const Z: Self = Self::new(0.0, 0.0, 1.0);

    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    #[must_use]
    pub const fn from_array(a: [f64; 3]) -> Self {
        Self::new(a[0], a[1], a[2])
    }

    #[must_use]
    pub const fn to_array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }

    /// Rounded to `f32`. Call it on offsets from a nearby origin, never on world positions.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn to_f32(self) -> [f32; 3] {
        [self.x as f32, self.y as f32, self.z as f32]
    }

    #[must_use]
    pub fn dot(self, o: Self) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    #[must_use]
    pub fn cross(self, o: Self) -> Self {
        Self::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }

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
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    pub m: [[f64; 4]; 4],
}

impl Mat4 {
    pub const IDENTITY: Self =
        Self { m: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]] };

    /// A view matrix: a camera at `eye` looking at `target`, right-handed, looking down its own −z.
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

    /// A perspective projection to wgpu's clip space (depth 0 at `near`, 1 at `far`).
    /// `fov_y` is the full vertical angle in radians; `aspect` is width over height.
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

    /// An orthographic projection to wgpu's clip space. `half_height` is half the visible height in
    /// metres; `aspect` is width over height.
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

    /// The homogeneous image of a point: `self · (p, 1)`.
    #[must_use]
    pub fn transform(&self, p: Vec3) -> [f64; 4] {
        let v = [p.x, p.y, p.z, 1.0];
        let mut out = [0.0; 4];
        for (row, o) in out.iter_mut().enumerate() {
            *o = (0..4).map(|col| self.m[col][row] * v[col]).sum();
        }
        out
    }

    /// A point taken to normalised device coordinates (x, y in −1…1, depth in 0…1), or `None` when it
    /// is not in front of the camera.
    #[must_use]
    pub fn project(&self, p: Vec3) -> Option<Vec3> {
        let [x, y, z, w] = self.transform(p);
        (w > 0.0).then(|| Vec3::new(x / w, y / w, z / w))
    }

    /// Rounded to `f32`, by columns, as a shader expects it.
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
        against: "a point ten times further away projects ten times nearer the axis",
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
