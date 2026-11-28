// The study's GPU kernels, written once in "N" arithmetic: a prelude (f32, float-float or f64)
// defines N and its operations; srbench's `gpu` binary prepends it. One thread per field point; every
// thread loops over the same elements [e0, e1), so each element is read once per workgroup.
//
// Buffers: data (elements, SLOTS per element; a slot is one f32, or an f32 pair (hi, lo) for ff and
// f64), points (6 f32 per point: x, y, z as hi/lo pairs), out (vec2<f32> per point, U at unit
// density summed so far: hi, lo). Every kernel adds its share to out.

struct Params { e0: u32, e1: u32, npts: u32, pad: u32 }
@group(0) @binding(0) var<storage, read> data: array<f32>;
@group(0) @binding(1) var<storage, read> pts: array<f32>;
@group(0) @binding(2) var<storage, read_write> outp: array<vec2<f32>>;
@group(0) @binding(3) var<uniform> prm: Params;

struct N3 { x: N, y: N, z: N }
fn n3(x: N, y: N, z: N) -> N3 { return N3(x, y, z); }
fn n3sub(a: N3, b: N3) -> N3 { return N3(nsub(a.x, b.x), nsub(a.y, b.y), nsub(a.z, b.z)); }
fn n3add(a: N3, b: N3) -> N3 { return N3(nadd(a.x, b.x), nadd(a.y, b.y), nadd(a.z, b.z)); }
fn n3scale(a: N3, s: N) -> N3 { return N3(nmul(a.x, s), nmul(a.y, s), nmul(a.z, s)); }
fn n3dot(a: N3, b: N3) -> N { return nadd(nadd(nmul(a.x, b.x), nmul(a.y, b.y)), nmul(a.z, b.z)); }
fn n3cross(a: N3, b: N3) -> N3 {
    return N3(nsub(nmul(a.y, b.z), nmul(a.z, b.y)), nsub(nmul(a.z, b.x), nmul(a.x, b.z)), nsub(nmul(a.x, b.y), nmul(a.y, b.x)));
}
fn n3len(a: N3) -> N { return nsqrt(n3dot(a, a)); }
fn ld3(e: u32, k: u32) -> N3 { return N3(ld(e, k), ld(e, k + 1u), ld(e, k + 2u)); }
fn point(i: u32) -> N3 {
    return N3(pt(pts[6u * i], pts[6u * i + 1u]), pt(pts[6u * i + 2u], pts[6u * i + 3u]), pt(pts[6u * i + 4u], pts[6u * i + 5u]));
}

// Edge log L = log1p(2ℓ/(a + b − ℓ)) and the solid-angle factor of one triangle.
fn edge_log(la: N, lb: N, l: N) -> N { return nlog1p(ndiv(nmul(nc(2.0), l), nsub(nadd(la, lb), l))); }

// ---------------------------------------------------------------- soup: 25 slots per triangle
// v0 v1 v2 (9), n (3), ne0 ne1 ne2 (9), el (3), 2A (1)
fn soup_u(e: u32, p: N3) -> N {
    let r0 = n3sub(ld3(e, 0u), p);
    let r1 = n3sub(ld3(e, 3u), p);
    let r2 = n3sub(ld3(e, 6u), p);
    let l0 = n3len(r0);
    let l1 = n3len(r1);
    let l2 = n3len(r2);
    let h = n3dot(ld3(e, 9u), r0);
    let den = nadd(nadd(nmul(nmul(l0, l1), l2), nmul(l0, n3dot(r1, r2))), nadd(nmul(l1, n3dot(r2, r0)), nmul(l2, n3dot(r0, r1))));
    let om = nmul(nc(2.0), natan2(nmul(h, ld(e, 24u)), den));
    var s = nmul(n3dot(ld3(e, 12u), r0), edge_log(l0, l1, ld(e, 21u)));
    s = nadd(s, nmul(n3dot(ld3(e, 15u), r1), edge_log(l1, l2, ld(e, 22u))));
    s = nadd(s, nmul(n3dot(ld3(e, 18u), r2), edge_log(l2, l0, ld(e, 23u))));
    return nmul(nmul(nc(0.5), h), nsub(s, nmul(h, om)));
}

// ---------------------------------------------------------------- PLIC: 8 slots per cell
// lo (3), h (1), n (3), alpha (1): the cell lo + [0, h]^3 cut to n·(x − centre) ≤ alpha, built
// here face by face (no stored polyhedron). Each cube face is clipped (at most 5 corners); its exit
// and entry points give one edge of the cap, which is closed by a signed fan from the first one.
// Corner k of the cube is (k & 1, (k >> 1) & 1, (k >> 2) & 1).
const FACE = array<vec4<u32>, 6>(
    vec4<u32>(0u, 2u, 3u, 1u), vec4<u32>(4u, 5u, 7u, 6u), vec4<u32>(0u, 1u, 5u, 4u),
    vec4<u32>(2u, 6u, 7u, 3u), vec4<u32>(0u, 4u, 6u, 2u), vec4<u32>(1u, 3u, 7u, 5u));
const AXIS = array<u32, 6>(2u, 2u, 1u, 1u, 0u, 0u);
const SIGN = array<f32, 6>(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0);

// One planar face's U share: polygon q[0..k) (offsets from the point), unit normal nf.
fn face_u(q: array<N3, 6>, k: u32, nf: N3) -> N {
    let h = n3dot(nf, q[0]);
    var len: array<N, 6>;
    for (var i = 0u; i < k; i++) { len[i] = n3len(q[i]); }
    var s = nc(0.0);
    for (var i = 0u; i < k; i++) {
        let j = select(i + 1u, 0u, i + 1u == k);
        let d = n3sub(q[j], q[i]);
        let l = n3len(d);
        if (nhi(l) > 0.0) {
            let ne = n3scale(n3cross(d, nf), ndiv(nc(1.0), l));
            s = nadd(s, nmul(n3dot(ne, q[i]), edge_log(len[i], len[j], l)));
        }
    }
    // Solid angle: 2 arg of the product of the fan's (D + i h 2A), scaled per face.
    let sc = ndiv(nc(1.0), nmul(nmul(len[0], len[1]), len[2]));
    var re = nc(1.0);
    var im = nc(0.0);
    for (var t = 1u; t + 1u < k; t++) {
        let a2 = n3dot(n3cross(n3sub(q[t], q[0]), n3sub(q[t + 1u], q[0])), nf);
        let l123 = nmul(nmul(len[0], len[t]), len[t + 1u]);
        let den = nmul(nadd(nadd(l123, nmul(len[0], n3dot(q[t], q[t + 1u]))), nadd(nmul(len[t], n3dot(q[t + 1u], q[0])), nmul(len[t + 1u], n3dot(q[0], q[t])))), sc);
        let num = nmul(nmul(h, a2), sc);
        let nr = nsub(nmul(re, den), nmul(im, num));
        im = nadd(nmul(re, num), nmul(im, den));
        re = nr;
    }
    let om = nmul(nc(2.0), natan2(im, re));
    return nmul(nmul(nc(0.5), h), nsub(s, nmul(h, om)));
}

fn plic_u(e: u32, p: N3) -> N {
    let lo = ld3(e, 0u);
    let h = ld(e, 3u);
    let n = ld3(e, 4u);
    let alpha = ld(e, 7u);
    let half = nmul(h, nc(0.5));
    let base = n3sub(lo, p);
    var cr: array<N3, 8>;
    var sd: array<N, 8>;
    for (var k = 0u; k < 8u; k++) {
        let o = N3(select(nc(0.0), h, (k & 1u) != 0u), select(nc(0.0), h, (k & 2u) != 0u), select(nc(0.0), h, (k & 4u) != 0u));
        cr[k] = n3add(base, o);
        sd[k] = nsub(n3dot(n, n3sub(o, N3(half, half, half))), alpha);
    }
    var u = nc(0.0);
    var cap_a: array<N3, 6>; // cap edges, already reversed: from entry to exit
    var cap_b: array<N3, 6>;
    var ncap = 0u;
    for (var f = 0u; f < 6u; f++) {
        let idx = FACE[f];
        var q: array<N3, 6>;
        var k = 0u;
        var exit_pt = cr[0];
        var entry_pt = cr[0];
        var cut = false;
        for (var i = 0u; i < 4u; i++) {
            let a = idx[i];
            let b = idx[(i + 1u) % 4u];
            let da = sd[a];
            let db = sd[b];
            if (nhi(da) <= 0.0) { q[k] = cr[a]; k++; }
            if ((nhi(da) <= 0.0) != (nhi(db) <= 0.0)) {
                let t = ndiv(da, nsub(da, db));
                let x = n3add(cr[a], n3scale(n3sub(cr[b], cr[a]), t));
                q[k] = x; k++;
                if (nhi(da) <= 0.0) { exit_pt = x; } else { entry_pt = x; }
                cut = true;
            }
        }
        if (k >= 3u) {
            var nf = N3(nc(0.0), nc(0.0), nc(0.0));
            let sg = nc(SIGN[f]);
            if (AXIS[f] == 0u) { nf.x = sg; } else if (AXIS[f] == 1u) { nf.y = sg; } else { nf.z = sg; }
            u = nadd(u, face_u(q, k, nf));
        }
        if (cut) { cap_a[ncap] = entry_pt; cap_b[ncap] = exit_pt; ncap++; }
    }
    if (ncap >= 3u) {
        // Each face's polygon runs exit -> entry along the cap plane, so the cap runs entry -> exit.
        let apex = cap_a[0];
        let la = n3len(apex);
        let h = n3dot(n, apex);
        var s = nc(0.0);
        var re = nc(1.0);
        var im = nc(0.0);
        let sc = ndiv(nc(1.0), nmul(nmul(la, la), la));
        for (var i = 0u; i < ncap; i++) {
            let a = cap_a[i];
            let b = cap_b[i];
            let lA = n3len(a);
            let lB = n3len(b);
            let d = n3sub(b, a);
            let l = n3len(d);
            if (nhi(l) > 0.0) {
                let ne = n3scale(n3cross(d, n), ndiv(nc(1.0), l));
                s = nadd(s, nmul(n3dot(ne, a), edge_log(lA, lB, l)));
                let a2 = n3dot(n3cross(n3sub(a, apex), n3sub(b, apex)), n);
                let l123 = nmul(nmul(la, lA), lB);
                let den = nmul(nadd(nadd(l123, nmul(la, n3dot(a, b))), nadd(nmul(lA, n3dot(b, apex)), nmul(lB, n3dot(apex, a)))), sc);
                let num = nmul(nmul(h, a2), sc);
                let nr = nsub(nmul(re, den), nmul(im, num));
                im = nadd(nmul(re, num), nmul(im, den));
                re = nr;
            }
        }
        let om = nmul(nc(2.0), natan2(im, re));
        u = nadd(u, nmul(nmul(nc(0.5), h), nsub(s, nmul(h, om))));
    }
    return u;
}

// ---------------------------------------------------------------- moments: 20 slots per element
// c (3), m (1), I (6: xx yy zz xy xz yz), T (10: xxx yyy zzz xxy xxz xyy yyz xzz yzz xyz)
fn mom_terms(e: u32, x: f32, y: f32, z: f32) -> f32 {
    // f32 terms of point + second + third moments, d = p − c given in f32.
    let r2 = x * x + y * y + z * z;
    let ir = 1.0 / sqrt(r2);
    let ir2 = ir * ir;
    let ir3 = ir2 * ir;
    let m = ldf(e, 3u);
    let i0 = ldf(e, 4u); let i1 = ldf(e, 5u); let i2 = ldf(e, 6u); let i3 = ldf(e, 7u); let i4 = ldf(e, 8u); let i5 = ldf(e, 9u);
    let tr = i0 + i1 + i2;
    let rir = i0 * x * x + i1 * y * y + i2 * z * z + 2.0 * (i3 * x * y + i4 * x * z + i5 * y * z);
    var u = m * ir + 0.5 * (3.0 * rir * ir2 - tr) * ir3;
    var t: array<f32, 10>;
    for (var k = 0u; k < 10u; k++) { t[k] = ldf(e, 10u + k); }
    let trrr = t[0] * x * x * x + t[1] * y * y * y + t[2] * z * z * z
        + 3.0 * (t[3] * x * x * y + t[4] * x * x * z + t[5] * x * y * y + t[6] * y * y * z + t[7] * x * z * z + t[8] * y * z * z)
        + 6.0 * t[9] * x * y * z;
    let rt = x * (t[0] + t[5] + t[7]) + y * (t[3] + t[1] + t[8]) + z * (t[4] + t[6] + t[2]);
    u = u + (2.5 * trrr * ir2 - 1.5 * rt) * ir3 * ir2;
    return u;
}

fn mom_u(e: u32, p: N3) -> N {
    // N arithmetic throughout.
    let d = n3sub(p, ld3(e, 0u));
    let r2 = n3dot(d, d);
    let ir = ndiv(nc(1.0), nsqrt(r2));
    let ir2 = nmul(ir, ir);
    let ir3 = nmul(ir2, ir);
    let tr = nadd(nadd(ld(e, 4u), ld(e, 5u)), ld(e, 6u));
    let x = d.x; let y = d.y; let z = d.z;
    var rir = nadd(nadd(nmul(ld(e, 4u), nmul(x, x)), nmul(ld(e, 5u), nmul(y, y))), nmul(ld(e, 6u), nmul(z, z)));
    rir = nadd(rir, nmul(nc(2.0), nadd(nadd(nmul(ld(e, 7u), nmul(x, y)), nmul(ld(e, 8u), nmul(x, z))), nmul(ld(e, 9u), nmul(y, z)))));
    var u = nadd(nmul(ld(e, 3u), ir), nmul(nmul(nc(0.5), nsub(nmul(nmul(nc(3.0), rir), ir2), tr)), ir3));
    let xx = nmul(x, x); let yy = nmul(y, y); let zz = nmul(z, z);
    var trrr = nadd(nadd(nmul(ld(e, 10u), nmul(xx, x)), nmul(ld(e, 11u), nmul(yy, y))), nmul(ld(e, 12u), nmul(zz, z)));
    var m3 = nadd(nadd(nmul(ld(e, 13u), nmul(xx, y)), nmul(ld(e, 14u), nmul(xx, z))), nmul(ld(e, 15u), nmul(x, yy)));
    m3 = nadd(m3, nadd(nadd(nmul(ld(e, 16u), nmul(yy, z)), nmul(ld(e, 17u), nmul(x, zz))), nmul(ld(e, 18u), nmul(y, zz))));
    trrr = nadd(nadd(trrr, nmul(nc(3.0), m3)), nmul(nc(6.0), nmul(ld(e, 19u), nmul(nmul(x, y), z))));
    let rt = nadd(nadd(nmul(x, nadd(nadd(ld(e, 10u), ld(e, 15u)), ld(e, 17u))), nmul(y, nadd(nadd(ld(e, 13u), ld(e, 11u)), ld(e, 18u)))),
        nmul(z, nadd(nadd(ld(e, 14u), ld(e, 16u)), ld(e, 12u))));
    u = nadd(u, nmul(nmul(nsub(nmul(nmul(nc(2.5), trrr), ir2), nmul(nc(1.5), rt)), ir3), ir2));
    return u;
}

fn mom_mixed_u(e: u32, p: N3) -> N {
    // The offset in N (tower-local), then f32 terms, summed in N.
    let d = n3sub(p, ld3(e, 0u));
    return nc(mom_terms(e, nhi(d.x), nhi(d.y), nhi(d.z)));
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= prm.npts) { return; }
    let p = point(i);
    var acc = nc(0.0);
    for (var e = prm.e0; e < prm.e1; e++) {
        acc = nadd(acc, KERNEL(e, p));
    }
    outp[i] = store(nadd(load(outp[i]), acc));
}
