// The forward-model kernels in WGSL (f32), mirroring `gravity`'s Rust functions statement-for-statement.
// Differential-first: the phase path differences potentials per element (never two large sums subtracted).

const G: f32 = 6.6743e-11;
const TAU: f32 = 6.283185307179586;

@group(0) @binding(0) var<storage, read> cloud: array<vec4<f32>>; // xyz = position, w = mass
@group(0) @binding(1) var<storage, read> params: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;

// V(p) = Σ mᵢ·(−G)/r   (gravity::potential)
fn potential_at(p: vec3<f32>) -> f32 {
    var acc = 0.0;
    let n = arrayLength(&cloud);
    for (var i = 0u; i < n; i = i + 1u) {
        let e = cloud[i];
        let d = p - e.xyz;
        let r = length(d);
        acc = acc + e.w * (-G) / r;
    }
    return acc;
}

// g(p) = Σ (−G·mᵢ)/r³ · d   (gravity::field)
fn field_at(p: vec3<f32>) -> vec3<f32> {
    var acc = vec3<f32>(0.0, 0.0, 0.0);
    let n = arrayLength(&cloud);
    for (var i = 0u; i < n; i = i + 1u) {
        let e = cloud[i];
        let d = p - e.xyz;
        let r = length(d);
        let coeff = (-G) * e.w / (r * r * r);
        acc = acc + d * coeff;
    }
    return acc;
}

// Ballistic free-fall with a single ∓v_rec π-pulse at τ = t_half (instrument::Arm::z_at).
fn ballistic(z: f32, v: f32, tau: f32, g: f32) -> f32 {
    return z + v * tau - 0.5 * g * tau * tau;
}
fn arm_z(z0: f32, v_first: f32, kick: f32, t_half: f32, g: f32, tau: f32) -> f32 {
    if (tau <= t_half) {
        return ballistic(z0, v_first, tau, g);
    }
    let z_t = ballistic(z0, v_first, t_half, g);
    let v_t = v_first - g * t_half + kick;
    return ballistic(z_t, v_t, tau - t_half, g);
}

@compute @workgroup_size(1)
fn k_potential() {
    out[0] = potential_at(vec3<f32>(params[0], params[1], params[2]));
}

@compute @workgroup_size(1)
fn k_field() {
    let g = field_at(vec3<f32>(params[0], params[1], params[2]));
    out[0] = g.x;
    out[1] = g.y;
    out[2] = g.z;
}

// Γ(p) = Σ coeff·(δ − 3·û·û),  coeff = (−G·mᵢ)/r³,  û = d/r   (gravity::gradient_tensor). Row-major out.
@compute @workgroup_size(1)
fn k_gamma() {
    let p = vec3<f32>(params[0], params[1], params[2]);
    for (var a = 0u; a < 9u; a = a + 1u) { out[a] = 0.0; }
    let n = arrayLength(&cloud);
    for (var i = 0u; i < n; i = i + 1u) {
        let e = cloud[i];
        let d = p - e.xyz;
        let r = length(d);
        let inv_r = 1.0 / r;
        let coeff = (-G) * e.w / (r * r * r);
        var u = array<f32, 3>(d.x * inv_r, d.y * inv_r, d.z * inv_r);
        for (var a = 0u; a < 3u; a = a + 1u) {
            for (var b = 0u; b < 3u; b = b + 1u) {
                let kron = select(0.0, 1.0, a == b);
                out[a * 3u + b] = out[a * 3u + b] + coeff * (kron - 3.0 * u[a] * u[b]);
            }
        }
    }
}

@compute @workgroup_size(1)
fn k_arm() {
    // params = [z0, v_first, kick, t_half, g, tau]
    out[0] = arm_z(params[0], params[1], params[2], params[3], params[4], params[5]);
}

// ── Phase integral (PropagationIntegral) — static source. Params layout:
//   [0]=m_a [1]=hbar [2]=g [3]=t_half [4]=v_rec [5]=u0 [6]=ifo_sep [7]=fine_dt
//   [8..12]=source quat (w,x,y,z)  [12..15]=source translation
//   [15..19]=detector quat         [19..22]=detector translation
var<private> cur_z0: f32;

fn iso_apply(q: vec4<f32>, tr: vec3<f32>, p: vec3<f32>) -> vec3<f32> {
    let s = q.x;
    let u = vec3<f32>(q.y, q.z, q.w);
    return 2.0 * dot(u, p) * u + (s * s - dot(u, u)) * p + 2.0 * s * cross(u, p) + tr;
}
fn iso_inv_apply(q: vec4<f32>, tr: vec3<f32>, p: vec3<f32>) -> vec3<f32> {
    // rotate (p − tr) by the conjugate quaternion (w, −x, −y, −z).
    let d = p - tr;
    let s = q.x;
    let u = vec3<f32>(-q.y, -q.z, -q.w);
    return 2.0 * dot(u, d) * u + (s * s - dot(u, u)) * d + 2.0 * s * cross(u, d);
}

// The differenced integrand Σ mᵢ(−G)(1/r_u − 1/r_l) — differential-first: the arms' potentials are
// differenced PER ELEMENT, never formed as two large sums and subtracted.
fn phase_integrand(flight: f32) -> f32 {
    let g = params[2];
    let t_half = params[3];
    let v_rec = params[4];
    let u0 = params[5];
    let src_q = vec4<f32>(params[8], params[9], params[10], params[11]);
    let src_t = vec3<f32>(params[12], params[13], params[14]);
    let det_q = vec4<f32>(params[15], params[16], params[17], params[18]);
    let det_t = vec3<f32>(params[19], params[20], params[21]);
    let z_u = arm_z(cur_z0, u0 + v_rec, -v_rec, t_half, g, flight);
    let z_l = arm_z(cur_z0, u0, v_rec, t_half, g, flight);
    let pu = iso_inv_apply(src_q, src_t, iso_apply(det_q, det_t, vec3<f32>(0.0, 0.0, z_u)));
    let pl = iso_inv_apply(src_q, src_t, iso_apply(det_q, det_t, vec3<f32>(0.0, 0.0, z_l)));
    var acc = 0.0;
    let n = arrayLength(&cloud);
    for (var i = 0u; i < n; i = i + 1u) {
        let e = cloud[i];
        let du = pu - e.xyz;
        let dl = pl - e.xyz;
        acc = acc + e.w * (-G) * (1.0 / length(du) - 1.0 / length(dl));
    }
    return acc;
}

// Composite Simpson over [a, b] at ≈step resolution (even interval count) — matches instrument::simpson.
fn simpson_phase(a: f32, b: f32, step: f32) -> f32 {
    var n = i32(round((b - a) / step));
    if (n < 2) { n = 2; }
    if ((n & 1) == 1) { n = n + 1; }
    let h = (b - a) / f32(n);
    var s = phase_integrand(a) + phase_integrand(b);
    for (var i = 1; i < n; i = i + 1) {
        let w = select(2.0, 4.0, (i & 1) == 1);
        s = s + w * phase_integrand(a + f32(i) * h);
    }
    return s * h / 3.0;
}

fn dphi_ifo(z0: f32) -> f32 {
    cur_z0 = z0;
    let m_a = params[0];
    let hbar = params[1];
    let t_half = params[3];
    let fine_dt = params[7];
    // Split the quadrature at the π-pulse (τ = T): the arm velocity kinks there.
    let acc = simpson_phase(0.0, t_half, fine_dt) + simpson_phase(t_half, 2.0 * t_half, fine_dt);
    return (m_a / hbar) * acc;
}

@compute @workgroup_size(1)
fn k_phase_static() {
    let ifo_sep = params[6];
    let d1 = dphi_ifo(0.0);
    let d2 = dphi_ifo(ifo_sep);
    out[0] = d2 - d1; // ΔΦ = δφ₂ − δφ₁
}

// ── Pass 1: the momentum-splitting free-rotation integrator (mirrors source::integrator::step). ──
// Quaternions are (w, x, y, z) stored as vec4 (.x=w, .y=x, .z=y, .w=z).
fn quat_hamilton(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(
        a.x * b.x - a.y * b.y - a.z * b.z - a.w * b.w,
        a.x * b.y + a.y * b.x + a.z * b.w - a.w * b.z,
        a.x * b.z - a.y * b.w + a.z * b.x + a.w * b.y,
        a.x * b.w + a.y * b.z - a.z * b.y + a.w * b.x,
    );
}
fn quat_axis_q(axis: u32, phi: f32) -> vec4<f32> {
    let s = sin(phi * 0.5);
    var q = vec4<f32>(cos(phi * 0.5), 0.0, 0.0, 0.0);
    if (axis == 0u) { q.y = s; } else if (axis == 1u) { q.z = s; } else { q.w = s; }
    return q;
}
fn rotate_pi_wgsl(pi: ptr<function, array<f32, 3>>, k: u32, phi: f32) {
    let c = cos(phi);
    let s = sin(phi);
    if (k == 0u) {
        let y = (*pi)[1]; let z = (*pi)[2];
        (*pi)[1] = y * c + z * s; (*pi)[2] = z * c - y * s;
    } else if (k == 1u) {
        let z = (*pi)[2]; let x = (*pi)[0];
        (*pi)[2] = z * c + x * s; (*pi)[0] = x * c - z * s;
    } else {
        let x = (*pi)[0]; let y = (*pi)[1];
        (*pi)[0] = x * c + y * s; (*pi)[1] = y * c - x * s;
    }
}
fn free_step(q: ptr<function, vec4<f32>>, omega: ptr<function, vec3<f32>>, inertia: vec3<f32>, h: f32) {
    let half = h * 0.5;
    var inv = array<f32, 3>(1.0 / inertia.x, 1.0 / inertia.y, 1.0 / inertia.z);
    var pi = array<f32, 3>(inertia.x * (*omega).x, inertia.y * (*omega).y, inertia.z * (*omega).z);
    var ks = array<u32, 5>(0u, 1u, 2u, 1u, 0u);
    var taus = array<f32, 5>(half, half, h, half, half);
    for (var i = 0u; i < 5u; i = i + 1u) {
        let k = ks[i];
        let phi = pi[k] * inv[k] * taus[i];
        rotate_pi_wgsl(&pi, k, phi);
        *q = quat_hamilton(*q, quat_axis_q(k, phi));
    }
    let n = sqrt((*q).x * (*q).x + (*q).y * (*q).y + (*q).z * (*q).z + (*q).w * (*q).w);
    *q = *q / n;
    *omega = vec3<f32>(pi[0] * inv[0], pi[1] * inv[1], pi[2] * inv[2]);
}
// Integrate from identity/ω₀ to t on the fine_dt grid: n full steps + a remainder step.
// f32 substep horizon: the residual vs the f64 CPU integrator is pure f32 accumulation, growing with
// step count — ≤1e-5 to ~200 substeps (2 s at fine_dt=0.01), ~1.7e-5 by 300, still well within
// cpu_equals_gpu's ≤1e-4 (pass1_ode_on_device). A very long rotating scenario would need a finer bound.
fn free_rotation_quat(omega0: vec3<f32>, inertia: vec3<f32>, t: f32, fine_dt: f32) -> vec4<f32> {
    var q = vec4<f32>(1.0, 0.0, 0.0, 0.0);
    var omega = omega0;
    let n = i32(max(t / fine_dt, 0.0));
    for (var i = 0; i < n; i = i + 1) {
        free_step(&q, &omega, inertia, fine_dt);
    }
    let rem = t - f32(n) * fine_dt;
    if (rem > 1e-15) {
        free_step(&q, &omega, inertia, rem);
    }
    return q;
}

@compute @workgroup_size(1)
fn k_free_rotation_pose() {
    // params = [ω0.x, ω0.y, ω0.z, I1, I2, I3, t, fine_dt]
    let q = free_rotation_quat(
        vec3<f32>(params[0], params[1], params[2]),
        vec3<f32>(params[3], params[4], params[5]),
        params[6],
        params[7],
    );
    out[0] = q.x; out[1] = q.y; out[2] = q.z; out[3] = q.w;
}

// ── General phase kernel: source pose per node (closed-form Path/Timing/Orient or the ODE), + atmo.
//   [0..9] instrument+t (as k_phase_static header but with t at [8])
//   [9..16] detector pose   [16..23] source placement
//   [23] path_kind  [24..27] a/axis  [27..30] b  [30]=amp [31]=freq [32]=phase
//   [33] Uniform rate   [34] orient_kind  [35..39] fixed quat  [39..42] ω0  [42..45] inertia
//   [45] n_modes   [46 + 6i ..] mode i: kx ky kz omega psi coeff
fn g_path_pos(u: f32) -> vec3<f32> {
    let kind = i32(params[23]);
    if (kind == 1) { // LinearPass: a + (b − a)·u
        let a = vec3<f32>(params[24], params[25], params[26]);
        let b = vec3<f32>(params[27], params[28], params[29]);
        return a + (b - a) * u;
    } else if (kind == 2) { // Oscillation: axis·amp·sin(2π·freq·u + phase)
        let axis = vec3<f32>(params[24], params[25], params[26]);
        return axis * (params[30] * sin(TAU * params[31] * u + params[32]));
    }
    return vec3<f32>(0.0, 0.0, 0.0); // Static
}
fn g_orient_quat(t: f32) -> vec4<f32> {
    if (i32(params[34]) == 1) { // FreeRotation
        return free_rotation_quat(
            vec3<f32>(params[39], params[40], params[41]),
            vec3<f32>(params[42], params[43], params[44]),
            t, params[7]);
    }
    return vec4<f32>(params[35], params[36], params[37], params[38]); // Fixed
}
struct Pose { q: vec4<f32>, tr: vec3<f32> }
fn g_source_pose(t_abs: f32) -> Pose {
    let place_q = vec4<f32>(params[16], params[17], params[18], params[19]);
    let place_t = vec3<f32>(params[20], params[21], params[22]);
    let p = g_path_pos(params[33] * t_abs); // Uniform timing u = rate·t
    let tr = iso_apply(place_q, place_t, p); // world translation
    let q_src = quat_hamilton(place_q, g_orient_quat(t_abs));
    return Pose(q_src, tr);
}
fn g_integrand(flight: f32) -> f32 {
    let two_t = 2.0 * params[3];
    let t_abs = (params[8] - two_t) + flight;
    let det_q = vec4<f32>(params[9], params[10], params[11], params[12]);
    let det_t = vec3<f32>(params[13], params[14], params[15]);
    let z_u = arm_z(cur_z0, params[5] + params[4], -params[4], params[3], params[2], flight);
    let z_l = arm_z(cur_z0, params[5], params[4], params[3], params[2], flight);
    let pu_w = iso_apply(det_q, det_t, vec3<f32>(0.0, 0.0, z_u));
    let pl_w = iso_apply(det_q, det_t, vec3<f32>(0.0, 0.0, z_l));
    // cloud sources in body frame (differential-first per element)
    let pose = g_source_pose(t_abs);
    let pu = iso_inv_apply(pose.q, pose.tr, pu_w);
    let pl = iso_inv_apply(pose.q, pose.tr, pl_w);
    var acc = 0.0;
    let n = arrayLength(&cloud);
    for (var i = 0u; i < n; i = i + 1u) {
        let e = cloud[i];
        acc = acc + e.w * (-G) * (1.0 / length(pu - e.xyz) - 1.0 / length(pl - e.xyz));
    }
    // atmospheric modes in world frame (differential-first per mode)
    let nm = i32(params[45]);
    for (var i = 0; i < nm; i = i + 1) {
        let base = 46 + 6 * i;
        let k = vec3<f32>(params[base], params[base + 1], params[base + 2]);
        let phase_c = params[base + 4] - params[base + 3] * t_abs; // ψ − ωt
        acc = acc + params[base + 5] * (cos(dot(k, pu_w) + phase_c) - cos(dot(k, pl_w) + phase_c));
    }
    return acc;
}
fn g_simpson(a: f32, b: f32, step: f32) -> f32 {
    var n = i32(round((b - a) / step));
    if (n < 2) { n = 2; }
    if ((n & 1) == 1) { n = n + 1; }
    let h = (b - a) / f32(n);
    var s = g_integrand(a) + g_integrand(b);
    for (var i = 1; i < n; i = i + 1) {
        let w = select(2.0, 4.0, (i & 1) == 1);
        s = s + w * g_integrand(a + f32(i) * h);
    }
    return s * h / 3.0;
}
fn g_dphi_ifo(z0: f32) -> f32 {
    cur_z0 = z0;
    let acc = g_simpson(0.0, params[3], params[7]) + g_simpson(params[3], 2.0 * params[3], params[7]);
    return (params[0] / params[1]) * acc;
}
@compute @workgroup_size(1)
fn k_phase() {
    let d1 = g_dphi_ifo(0.0);
    let d2 = g_dphi_ifo(params[6]);
    out[0] = d2 - d1; // ΔΦ = δφ₂ − δφ₁
}
