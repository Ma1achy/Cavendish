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
