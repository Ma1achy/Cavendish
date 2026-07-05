//! A minimal orbit camera → column-major `view_proj` for the renderer's uniform. Deliberately small:
//! the viewer inspects one scenario, so there is no camera-path machinery here (scope, `viewer.md` §6).

/// An orbit camera looking at `target` from `eye`, right-handed, wgpu clip space (`z ∈ [0, 1]`).
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    pub fovy: f32,
    pub aspect: f32,
    pub znear: f32,
    pub zfar: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            eye: [6.0, 4.0, 8.0],
            target: [0.0, 0.0, 0.0],
            up: [0.0, 0.0, 1.0],
            fovy: 0.9,
            aspect: 1.0,
            znear: 0.05,
            zfar: 1000.0,
        }
    }
}

impl Camera {
    /// The combined `proj · view` as a column-major 4×4 (columns are the array's outer index) — the
    /// layout a WGSL `mat4x4<f32>` expects from a `bytemuck`-cast uniform.
    pub fn view_proj(&self) -> [[f32; 4]; 4] {
        mul(
            perspective_rh(self.fovy, self.aspect, self.znear, self.zfar),
            look_at_rh(self.eye, self.target, self.up),
        )
    }

    /// Point the camera from an orbit about `target`: `azimuth` around the up axis (`+z`), `elevation`
    /// above the horizontal plane, at `radius`. Drives the App's drag-to-orbit / scroll-to-zoom.
    pub fn set_orbit(&mut self, target: [f32; 3], azimuth: f32, elevation: f32, radius: f32) {
        self.target = target;
        self.eye = orbit_eye(target, azimuth, elevation, radius);
    }
}

/// The eye position for an orbit about `target`: `azimuth` around `+z`, `elevation` above the `xy`
/// plane (clamped just off the poles so the up vector never degenerates), at `radius`.
pub fn orbit_eye(target: [f32; 3], azimuth: f32, elevation: f32, radius: f32) -> [f32; 3] {
    let el = elevation.clamp(-1.5533, 1.5533); // ±89° in radians
    let (ce, se) = (el.cos(), el.sin());
    let (ca, sa) = (azimuth.cos(), azimuth.sin());
    [
        target[0] + radius * ce * ca,
        target[1] + radius * ce * sa,
        target[2] + radius * se,
    ]
}

/// Project a world point to a pixel in an image `rect` (`[0,0]` at the rect's top-left) via `view_proj`.
/// Returns `None` when the point is at or behind the camera plane (`w ≤ 0`) — nothing to draw.
pub fn project(view_proj: &[[f32; 4]; 4], world: [f32; 3], rect: [f32; 4]) -> Option<[f32; 2]> {
    let m = view_proj;
    let clip = [
        m[0][0] * world[0] + m[1][0] * world[1] + m[2][0] * world[2] + m[3][0],
        m[0][1] * world[0] + m[1][1] * world[1] + m[2][1] * world[2] + m[3][1],
        m[0][3] * world[0] + m[1][3] * world[1] + m[2][3] * world[2] + m[3][3], // w
    ];
    let w = clip[2];
    if w <= 1e-6 {
        return None;
    }
    let ndc = [clip[0] / w, clip[1] / w]; // ∈ [-1, 1], y up
    let (x0, y0, width, height) = (rect[0], rect[1], rect[2], rect[3]);
    Some([
        x0 + (ndc[0] * 0.5 + 0.5) * width,
        y0 + (1.0 - (ndc[1] * 0.5 + 0.5)) * height, // flip y: screen y grows downward
    ])
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt();
    if l > 0.0 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        a
    }
}

/// Right-handed look-at (camera faces `−z`), column-major.
fn look_at_rh(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let f = norm(sub(target, eye));
    let s = norm(cross(f, up));
    let u = cross(s, f);
    [
        [s[0], u[0], -f[0], 0.0],
        [s[1], u[1], -f[1], 0.0],
        [s[2], u[2], -f[2], 0.0],
        [-dot(s, eye), -dot(u, eye), dot(f, eye), 1.0],
    ]
}

/// Right-handed perspective with wgpu clip depth (`z ∈ [0, 1]`), column-major.
fn perspective_rh(fovy: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let g = 1.0 / (0.5 * fovy).tan();
    [
        [g / aspect, 0.0, 0.0, 0.0],
        [0.0, g, 0.0, 0.0],
        [0.0, 0.0, far / (near - far), -1.0],
        [0.0, 0.0, near * far / (near - far), 0.0],
    ]
}

/// Column-major 4×4 product `a · b`.
fn mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut m = [[0.0f32; 4]; 4];
    for (c, col) in m.iter_mut().enumerate() {
        for (r, cell) in col.iter_mut().enumerate() {
            *cell = a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3];
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbit_eye_spherical() {
        let t = [1.0, 2.0, 3.0];
        // azimuth 0, elevation 0 ⇒ along +x at `radius`.
        let e = orbit_eye(t, 0.0, 0.0, 5.0);
        assert!(
            (e[0] - 6.0).abs() <= 1e-5 && (e[1] - 2.0).abs() <= 1e-5 && (e[2] - 3.0).abs() <= 1e-5
        );
        // azimuth 90° ⇒ along +y.
        let e = orbit_eye(t, std::f32::consts::FRAC_PI_2, 0.0, 5.0);
        assert!((e[0] - 1.0).abs() <= 1e-5 && (e[1] - 7.0).abs() <= 1e-5);
        // straight up (clamped just off the pole) ⇒ mostly +z.
        let e = orbit_eye(t, 0.0, std::f32::consts::FRAC_PI_2, 5.0);
        assert!(e[2] > 3.0 + 4.99, "near the +z pole: {e:?}");
    }

    #[test]
    fn project_target_to_centre() {
        // The look-at target projects to the image centre (NDC origin); a point behind the camera is None.
        let cam = Camera::default();
        let vp = cam.view_proj();
        let rect = [0.0, 0.0, 100.0, 80.0];
        let c = project(&vp, cam.target, rect).expect("target is in front");
        assert!(
            (c[0] - 50.0).abs() <= 0.5 && (c[1] - 40.0).abs() <= 0.5,
            "centre: {c:?}"
        );
        // 2·eye − target sits on the far side of the camera from the target ⇒ behind ⇒ None.
        let behind = [
            2.0 * cam.eye[0] - cam.target[0],
            2.0 * cam.eye[1] - cam.target[1],
            2.0 * cam.eye[2] - cam.target[2],
        ];
        assert!(
            project(&vp, behind, rect).is_none(),
            "behind the camera culls"
        );
    }
}
