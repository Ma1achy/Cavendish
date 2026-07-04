// The forward-model kernels in WGSL (f32), mirroring `gravity`'s Rust functions statement-for-statement.
// Differential-first: the phase path differences potentials per element (never two large sums subtracted).

const G: f32 = 6.6743e-11;

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
