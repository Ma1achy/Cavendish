//! The polyhedral kernel's tests: rows P1–P7 of `docs/slices/S5.md` §3, each with its control.

use super::*;
use crate::voxel::kernel::{cube_difference, cube_series, prism_potential};
use crate::voxel::mesh::torus_mesh;
use crate::voxel::shape::Shape;

use std::f64::consts::PI;

/// An odd, non-dyadic point, metres: where test bodies are put so that nothing rounds exactly.
const ODD: Vec3 = Vec3::new(0.123_456_7, -0.314_159_3, 0.271_828_2);

/// `n` directions spread over the sphere (a golden spiral offset off the axes).
fn directions(n: usize) -> Vec<Vec3> {
    let golden = PI * (3.0 - 5f64.sqrt());
    (0..n)
        .map(|k| {
            let z = 1.0 - 2.0 * (k as f64 + 0.37) / n as f64;
            let r = (1.0 - z * z).sqrt();
            let a = golden * (k as f64 + 0.21);
            Vec3::new(r * a.cos(), r * a.sin(), z)
        })
        .collect()
}

/// The fractional part of `k` times the golden ratio's inverse: a number in `[0, 1)` per `k`.
fn spread(k: usize) -> f64 {
    let x = (k as f64 + 0.5) * 0.618_033_988_749_894_9;
    x - x.floor()
}

fn relative(got: f64, want: f64) -> f64 {
    (got - want).abs() / want.abs()
}

fn relative_vec(got: Vec3, want: Vec3) -> f64 {
    (got - want).length() / want.length()
}

/// A rotation by `angle` radians about the unit `axis` (Rodrigues), applied to `v`.
fn turned(v: Vec3, axis: Vec3, angle: f64) -> Vec3 {
    let (s, c) = angle.sin_cos();
    v * c + axis.cross(v) * s + axis * (axis.dot(v) * (1.0 - c))
}

/// The axis and angle of the turned box.
fn turn() -> (Vec3, f64) {
    (Vec3::new(0.3, -0.5, 0.81).normalised(), 0.734)
}

/// A box of half-sizes `half` turned by [`turn`] about its centre, centred at `centre`.
fn turned_box(centre: Vec3, half: Vec3) -> Faceted {
    let (vertices, triangles) = box_mesh(-half, half);
    let (axis, angle) = turn();
    let vertices = vertices.iter().map(|v| centre + turned(*v, axis, angle)).collect();
    Faceted::indexed(vertices, &triangles).unwrap()
}

/// The solid with face 0's normal flipped (and so its `h` and `ω`): a broken solid for controls.
fn flipped(solid: &Faceted) -> Faceted {
    let mut broken = solid.clone();
    broken.faces[0].normal = -broken.faces[0].normal;
    broken
}

// ---------------------------------------------------------------------------------------------
// P1: the polyhedron is the prism.

/// The gradient of Nagy's corner term `F(x, y, z)` (the term of `voxel::kernel::prism_potential`)
/// in its offsets: `∂F/∂x = y ln(z + r) + z ln(y + r) − x atan(yz/(xr))` and its cycles. The
/// logarithm's argument is formed without cancellation as the kernel's is.
fn corner_gradient(x: f64, y: f64, z: f64) -> Vec3 {
    let r = (x * x + y * y + z * z).sqrt();
    let log = |u: f64, others: f64| {
        let sum = u + r;
        let sum = if u < 0.0 && sum < 1e-6 * r { others / (r - u) } else { sum };
        sum.ln()
    };
    let angle = |p: f64, q: f64, w: f64| if p == 0.0 { 0.0 } else { p * (q * w / (p * r)).atan() };
    let (xx, yy, zz) = (x * x, y * y, z * z);
    Vec3::new(
        y * log(z, xx + yy) + z * log(y, zz + xx) - angle(x, y, z),
        z * log(x, yy + zz) + x * log(z, xx + yy) - angle(y, z, x),
        x * log(y, zz + xx) + y * log(x, yy + zz) - angle(z, x, y),
    )
}

/// The exact attraction `g/G = −∇V/G` (kg/m²) of the prism of `density` kg/m³ spanning `lo` to
/// `hi` (its corners as offsets from the point, metres): Nagy's closed form, differentiated.
fn prism_attraction(density: f64, lo: Vec3, hi: Vec3) -> Vec3 {
    let mut total = Vec3::ZERO;
    for (i, x) in [lo.x, hi.x].into_iter().enumerate() {
        for (j, y) in [lo.y, hi.y].into_iter().enumerate() {
            for (k, z) in [lo.z, hi.z].into_iter().enumerate() {
                let sign = if (i + j + k) % 2 == 0 { -1.0 } else { 1.0 };
                total = total + corner_gradient(x, y, z) * sign;
            }
        }
    }
    total * -density
}

const BOX_HALF: Vec3 = Vec3::new(0.31, 0.47, 0.23);
const DENSITY: f64 = 2650.0;

/// 64 general points within three sides (the box's largest) of its centre, inside it included.
fn prism_points() -> Vec<Vec3> {
    let side = 2.0 * BOX_HALF.y;
    directions(64)
        .into_iter()
        .enumerate()
        .map(|(k, n)| ODD + n * (side * (0.15 + 2.85 * spread(k))))
        .collect()
}

/// The worst relative error in `V` and in `g` of `solid` against the prism, over the points.
fn against_the_prism(solid: &Faceted) -> (f64, f64) {
    let (lo, hi) = (ODD - BOX_HALF, ODD + BOX_HALF);
    let (mut worst_v, mut worst_g) = (0f64, 0f64);
    for p in prism_points() {
        let field = solid.field(DENSITY, p);
        let v = prism_potential(DENSITY, lo - p, hi - p);
        let g = prism_attraction(DENSITY, lo - p, hi - p);
        worst_v = worst_v.max(relative(field.potential, v));
        worst_g = worst_g.max(relative_vec(field.attraction, g));
    }
    (worst_v, worst_g)
}

fn the_box() -> Faceted {
    Faceted::from_cuboid(&Cuboid { centre: ODD, half: BOX_HALF }).unwrap()
}

cav_validate::case! {
    test: validation_the_polyhedron_is_the_prism,
    name: "polyhedral.the_prism",
    subsystem: "polyhedral",
    slice: "S5",
    reference: ClosedForm,
    against: "Nagy's prism (V) and its exact attraction (g, Nagy's corner term differentiated) for a 0.62×0.94×0.46 m box at a non-dyadic centre, 64 general points from 0.15 to 3 sides, inside included: the worst relative error of V and of g",
    unit: Relative,
    tolerance: 1e-13,
    measure: {
        let (v, g) = against_the_prism(&the_box());
        cav_validate::Measured::Error(v.max(g))
    },
}

#[test]
fn the_prism_attraction_is_the_prism_s_gradient_and_a_flipped_face_is_not_the_prism() {
    // The reference itself: the exact prism attraction is minus the gradient of Nagy's potential
    // (a five-point difference, step 1e-4 of the box).
    let (lo, hi) = (ODD - BOX_HALF, ODD + BOX_HALF);
    let v = |p: Vec3| prism_potential(DENSITY, lo - p, hi - p);
    for p in prism_points().into_iter().take(16) {
        let h = 1e-4;
        let along = |axis: Vec3| {
            -(-v(p + axis * (2.0 * h)) + 8.0 * v(p + axis * h) - 8.0 * v(p - axis * h)
                + v(p - axis * (2.0 * h)))
                / (12.0 * h)
        };
        let numerical = Vec3::new(along(Vec3::X), along(Vec3::Y), along(Vec3::Z));
        let exact = prism_attraction(DENSITY, lo - p, hi - p);
        assert!(relative_vec(exact, numerical) < 1e-8, "{p:?}: {exact:?} against {numerical:?}");
    }
    // Control: one face's normal flipped is far from the prism.
    let (v, g) = against_the_prism(&flipped(&the_box()));
    assert!(v > 1e-13 && g > 1e-13, "a flipped face still reads as the prism: {v:e} {g:e}");
    assert!(v > 1e-3, "{v:e}");
}

// ---------------------------------------------------------------------------------------------
// P2: the far field keeps its digits.

/// The far-field cube: side 1/64 m at a non-dyadic centre.
const FAR_SIDE: f64 = 1.0 / 64.0;

/// `V` and `g` by the face form with the textbook numerator `r₁·(r₂×r₃)` in place of
/// `h_f·2A_f`, everything else as [`Faceted::field`]: the control of P2.
fn triple_product_field(solid: &Faceted, density: f64, point: Vec3) -> (f64, Vec3) {
    let r: Vec<(Vec3, f64)> = solid.vertices.iter().map(|v| (*v - point, (*v - point).length())).collect();
    let (mut u, mut g) = (0.0, Vec3::ZERO);
    for f in &solid.faces {
        let [(r1, l1), (r2, l2), (r3, l3)] = f.corners.map(|c| r[c as usize]);
        let h = f.normal.dot(r1);
        let den = l1 * l2 * l3 + l1 * r2.dot(r3) + l2 * r3.dot(r1) + l3 * r1.dot(r2);
        let w = 2.0 * r1.dot(r2.cross(r3)).atan2(den);
        let mut s = 0.0;
        for k in 0..3 {
            let e = solid.edges[f.edges[k] as usize];
            let (ra, a) = r[e.ends[0] as usize];
            let (rb, b) = r[e.ends[1] as usize];
            let log = edge_log(ra, rb, a, b, e.direction, e.length);
            s += f.edge_normals[k].dot(r[f.corners[k] as usize].0) * log;
        }
        let b = s - h * w;
        u += 0.5 * h * b;
        g = g - f.normal * b;
    }
    (-density * u, g * density)
}

/// The cube series (to order 16: converged to rounding from 10 sides) and its gradient by a
/// five-point difference (step 1e-3 of the distance: truncation ~1e-12, rounding ~1e-13).
fn series_field(mass: f64, d: Vec3) -> (f64, Vec3) {
    let v = |d: Vec3| cube_series(16, mass, FAR_SIDE, d);
    let h = 1e-3 * d.length();
    let along = |axis: Vec3| {
        -(-v(d + axis * (2.0 * h)) + 8.0 * v(d + axis * h) - 8.0 * v(d - axis * h) + v(d - axis * (2.0 * h)))
            / (12.0 * h)
    };
    (v(d), Vec3::new(along(Vec3::X), along(Vec3::Y), along(Vec3::Z)))
}

/// The worst relative error in `V` and in `g` at `R/s = ratio` over 16 general directions, of
/// the kernel and of the triple-product control.
fn far_field_error(ratio: f64) -> (f64, f64) {
    let cube =
        Faceted::from_cuboid(&Cuboid { centre: ODD, half: Vec3::new(0.5, 0.5, 0.5) * FAR_SIDE }).unwrap();
    let mass = DENSITY * FAR_SIDE.powi(3);
    let (mut kernel, mut control) = (0f64, 0f64);
    for n in directions(16) {
        let d = n * (ratio * FAR_SIDE);
        let (v, g) = series_field(mass, d);
        let field = cube.field(DENSITY, ODD + d);
        kernel = kernel.max(relative(field.potential, v)).max(relative_vec(field.attraction, g));
        let (tv, tg) = triple_product_field(&cube, DENSITY, ODD + d);
        control = control.max(relative(tv, v)).max(relative_vec(tg, g));
    }
    (kernel, control)
}

cav_validate::case! {
    test: validation_polyhedral_far_field,
    name: "polyhedral.far_field",
    subsystem: "polyhedral",
    slice: "S5",
    reference: ClosedForm,
    against: "the cube series (order 16) and its five-point gradient, a 1/64 m cube at a non-dyadic centre, 16 general directions at R/s = 10, 30, 100, 300, 1000: the worst relative error of V and of g",
    unit: Relative,
    tolerance: 1e-9,
    measure: cav_validate::Measured::Error(
        [10.0, 30.0, 100.0, 300.0, 1000.0].into_iter().map(|r| far_field_error(r).0).fold(0.0, f64::max)
    ),
}

cav_validate::case! {
    test: validation_polyhedral_far_field_at_ten_thousand,
    name: "polyhedral.far_field_at_ten_thousand",
    subsystem: "polyhedral",
    slice: "S5",
    reference: ClosedForm,
    against: "P2 at R/s = 1e4: the cube series (order 16) and its five-point gradient, a 1/64 m cube at a non-dyadic centre, 16 general directions: the worst relative error of V and of g (the triple-product numerator is the control, in the_far_field_at_ten_thousand_and_the_triple_product_control)",
    unit: Relative,
    tolerance: 1e-7,
    measure: cav_validate::Measured::Error(far_field_error(1e4).0),
}

#[test]
fn the_far_field_at_ten_thousand_and_the_triple_product_control() {
    let (kernel, control) = far_field_error(1e4);
    assert!(kernel <= 1e-7, "at R/s = 1e4: {kernel:e}");
    // Control: the textbook numerator is at least ten times worse there.
    assert!(control >= 10.0 * kernel, "the triple product is not worse: {control:e} against {kernel:e}");
}

/// `V` and `g` by the kernel's face form (`h_f·2A_f` numerator) with each edge's logarithm the
/// naive ratio `ln((a + b + ℓ)/(a + b − ℓ))` in place of [`edge_log`]: the paper's third variant
/// (`docs/papers/polyhedral-gravity`, Figure "far field"). Not the kernel; for the figure only.
fn naive_log_field(solid: &Faceted, density: f64, point: Vec3) -> (f64, Vec3) {
    let r: Vec<(Vec3, f64)> = solid.vertices.iter().map(|v| (*v - point, (*v - point).length())).collect();
    let (mut u, mut g) = (0.0, Vec3::ZERO);
    for f in &solid.faces {
        let [(r1, l1), (r2, l2), (r3, l3)] = f.corners.map(|c| r[c as usize]);
        let h = f.normal.dot(r1);
        let den = l1 * l2 * l3 + l1 * r2.dot(r3) + l2 * r3.dot(r1) + l3 * r1.dot(r2);
        let w = 2.0 * (h * f.twice_area).atan2(den);
        let mut s = 0.0;
        for k in 0..3 {
            let e = solid.edges[f.edges[k] as usize];
            let (a, b) = (r[e.ends[0] as usize].1, r[e.ends[1] as usize].1);
            let log = ((a + b + e.length) / (a + b - e.length)).ln();
            s += f.edge_normals[k].dot(r[f.corners[k] as usize].0) * log;
        }
        let b = s - h * w;
        u += 0.5 * h * b;
        g = g - f.normal * b;
    }
    (-density * u, g * density)
}

/// The far-field columns of the paper's figure at `R/s = ratio`, the worst over 16 general
/// directions: `[A:V, A:g, B:V, B:g, C:V, C:g]`, relative, for (A) the triple-product numerator,
/// (B) the kernel and (C) the kernel's numerator with the naive logarithm.
fn far_field_columns(ratio: f64) -> [f64; 6] {
    let cube =
        Faceted::from_cuboid(&Cuboid { centre: ODD, half: Vec3::new(0.5, 0.5, 0.5) * FAR_SIDE }).unwrap();
    let mass = DENSITY * FAR_SIDE.powi(3);
    let mut out = [0f64; 6];
    for n in directions(16) {
        let d = n * (ratio * FAR_SIDE);
        let (v, g) = series_field(mass, d);
        let field = cube.field(DENSITY, ODD + d);
        let variants = [
            triple_product_field(&cube, DENSITY, ODD + d),
            (field.potential, field.attraction),
            naive_log_field(&cube, DENSITY, ODD + d),
        ];
        for (k, (tv, tg)) in variants.into_iter().enumerate() {
            out[2 * k] = out[2 * k].max(relative(tv, v));
            out[2 * k + 1] = out[2 * k + 1].max(relative_vec(tg, g));
        }
    }
    out
}

/// Where the paper's data files go: `docs/papers/polyhedral-gravity/data/cavendish/`.
fn paper_data() -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/papers/polyhedral-gravity/data/cavendish");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The far-field error by distance, printed and written as the paper's `farfield.txt`
/// (`make.py farfield`).
#[test]
#[ignore = "a diagnostic: the far-field error by distance; writes the paper's data/cavendish/farfield.txt"]
fn far_field_by_distance() {
    let mut text = String::from("R/s       A:U rel    A:g rel    B:U rel    B:g rel    C:U rel    C:g rel\n");
    for ratio in [10.0, 30.0, 100.0, 300.0, 1e3, 3e3, 1e4, 3e4, 1e5, 3e5] {
        let c = far_field_columns(ratio);
        println!(
            "R/s {ratio:8.0e}: kernel {:.1e}, triple product {:.1e}, naive log {:.1e}",
            c[3], c[1], c[5]
        );
        text += &format!("{ratio:8.0e}");
        for x in c {
            text += &format!("   {x:8.1e}");
        }
        text += "\n";
    }
    std::fs::write(paper_data().join("farfield.txt"), text).unwrap();
}

// ---------------------------------------------------------------------------------------------
// P3: beside an edge.

/// The near-edge cube: side 1/64 m, its centre non-dyadic and near the origin (so that the
/// rounding of the point's own position, `ε|p|/(d·L)`, stays below the criterion).
const NEAR_CENTRE: Vec3 = Vec3::new(0.012_345_7, -0.008_141_6, 0.005_718_3);

/// At `d` metres from the middle of the cube's edge at `x = y = +s/2`, outwards across it: the
/// error of `L` (the kernel's form and the direct `a + b − ℓ`) against `2 asinh(ℓ/2d)`, and the
/// error of `V` and `g` against the prism.
fn near_edge(d: f64) -> (f64, f64, f64, f64) {
    let h = 0.5 * FAR_SIDE;
    let cube = Faceted::from_cuboid(&Cuboid { centre: NEAR_CENTRE, half: Vec3::new(h, h, h) }).unwrap();
    let middle = NEAR_CENTRE + Vec3::new(h, h, 0.0);
    let p = middle + Vec3::new(0.8, 0.6, 0.0) * d;
    let edge = cube
        .edges
        .iter()
        .find(|e| {
            let mid = (cube.vertices[e.ends[0] as usize] + cube.vertices[e.ends[1] as usize]) * 0.5;
            (mid - middle).length() < 1e-12
        })
        .copied()
        .expect("the cube has the edge");
    let (ra, rb) = (cube.vertices[edge.ends[0] as usize] - p, cube.vertices[edge.ends[1] as usize] - p);
    let (a, b) = (ra.length(), rb.length());
    let exact = 2.0 * (0.5 * edge.length / d).asinh();
    let kernel = relative(edge_log(ra, rb, a, b, edge.direction, edge.length), exact);
    let gap = a + b - edge.length;
    let direct = if gap > 0.0 { (2.0 * edge.length / gap).ln_1p() } else { f64::INFINITY };
    let direct = relative(direct, exact);
    let (lo, hi) = (NEAR_CENTRE - Vec3::new(h, h, h) - p, NEAR_CENTRE + Vec3::new(h, h, h) - p);
    let field = cube.field(DENSITY, p);
    let v = relative(field.potential, prism_potential(DENSITY, lo, hi));
    let g = relative_vec(field.attraction, prism_attraction(DENSITY, lo, hi));
    (kernel, direct, v, g)
}

const NEAR_DISTANCES: [f64; 5] = [1e-4, 1e-5, 1e-6, 1e-7, 1e-8];

cav_validate::case! {
    test: validation_polyhedral_near_edge,
    name: "polyhedral.near_edge",
    subsystem: "polyhedral",
    slice: "S5",
    reference: ClosedForm,
    against: "points 1e-4 to 1e-8 m across the middle of an edge of a 1/64 m cube: the edge's logarithm against 2·asinh(ℓ/2d), V and g against Nagy's prism and its exact attraction; the worst relative error",
    unit: Relative,
    tolerance: 1e-10,
    measure: cav_validate::Measured::Error(NEAR_DISTANCES.into_iter().map(|d| {
        let (l, _, v, g) = near_edge(d);
        l.max(v).max(g)
    }).fold(0.0, f64::max)),
}

#[test]
fn beside_an_edge_the_direct_form_loses_its_digits() {
    // Control: the direct a + b − ℓ loses at least 1e-7 of L at 1e-8 m.
    let (kernel, direct, _, _) = near_edge(1e-8);
    assert!(direct >= 1e-7, "the direct form kept its digits: {direct:e} (kernel {kernel:e})");
}

/// The near-edge errors by distance, printed and written as the paper's `nearedge.txt`
/// (`make.py nearedge`): `d`, the exact `L`, the direct form's and the kernel's relative error, and
/// `|p|`, the point's distance from the origin (the input-rounding floor `ε|p|/(d·L)` is drawn from
/// it). A direct form that divides by zero is written `inf`.
#[test]
#[ignore = "a diagnostic: the near-edge errors by distance; writes the paper's data/cavendish/nearedge.txt"]
fn near_edge_by_distance() {
    let mut text = String::from("d          L exact      naive rel    hk rel       |p|\n");
    for d in [1e-2, 1e-3, 1e-4, 1e-5, 1e-6, 1e-7, 1e-8, 1e-9, 1e-10, 1e-12, 1e-14] {
        let (l, direct, v, g) = near_edge(d);
        println!("d {d:6.0e}: L {l:.1e} (direct {direct:.1e}), V {v:.1e}, g {g:.1e}");
        let h = 0.5 * FAR_SIDE;
        let p = NEAR_CENTRE + Vec3::new(h, h, 0.0) + Vec3::new(0.8, 0.6, 0.0) * d;
        let exact = 2.0 * (0.5 * FAR_SIDE / d).asinh();
        let direct = if direct.is_finite() { format!("{direct:9.1e}") } else { format!("{:>9}", "inf") };
        text += &format!("{d:8.0e}   {exact:10.4}   {direct}    {l:9.1e}    {:.6}\n", p.length());
    }
    std::fs::write(paper_data().join("nearedge.txt"), text).unwrap();
}

// ---------------------------------------------------------------------------------------------
// P4: finite and continuous on the surface.

/// Places on the surface of `solid` and a direction across each: a point on a face, on an edge,
/// at a vertex and on an edge's extension past its end.
fn surface_places(solid: &Faceted) -> Vec<(&'static str, Vec3, Vec3)> {
    let f = solid.faces[3];
    let [a, b, c] = f.corners.map(|i| solid.vertices[i as usize]);
    let centroid = solid.moments(1.0).centre;
    let on_face = a * 0.21 + b * 0.33 + c * 0.46;
    let on_edge = a * 0.63 + b * 0.37;
    let e = solid.edges[f.edges[0] as usize];
    let eb = solid.vertices[e.ends[1] as usize];
    let extension = eb + e.direction * (0.29 * e.length);
    // Across the edge: away from the centroid, square to the edge.
    let out = on_edge - centroid;
    let across_edge = (out - e.direction * out.dot(e.direction)).normalised();
    let beside = {
        let n = f.normal;
        (n - e.direction * n.dot(e.direction)).normalised()
    };
    vec![
        ("a face", on_face, f.normal),
        ("an edge", on_edge, across_edge),
        ("a vertex", a, (a - centroid).normalised()),
        ("an edge's extension", extension, beside),
    ]
}

/// The worst relative change of `V` and `g` between each surface place and points `1e-9·s` to
/// either side; panics on a value that is not finite.
fn surface_continuity(solid: &Faceted) -> f64 {
    let mut worst = 0f64;
    for (what, p, across) in surface_places(solid) {
        let at = solid.field(DENSITY, p);
        assert!(at.potential.is_finite(), "{what}: V = {}", at.potential);
        assert!(at.attraction.to_array().iter().all(|x| x.is_finite()), "{what}: g = {:?}", at.attraction);
        for sign in [-1.0, 1.0] {
            let near = solid.field(DENSITY, p + across * (sign * 1e-9 * solid.size()));
            worst = worst.max(relative(near.potential, at.potential));
            worst = worst.max(relative_vec(near.attraction, at.attraction));
        }
    }
    worst
}

fn surface_bodies() -> Vec<Faceted> {
    vec![
        Faceted::from_cuboid(&Cuboid { centre: ODD, half: BOX_HALF }).unwrap(),
        Faceted::from_polyhedron(&Platonic::Icosahedron.solid(0.83).moved(ODD)).unwrap(),
        turned_box(ODD, BOX_HALF),
    ]
}

#[test]
fn the_polyhedron_is_finite_on_its_surface() {
    for solid in surface_bodies() {
        let worst = surface_continuity(&solid);
        assert!(worst <= 1e-7, "a jump at the surface: {worst:e}");
    }
    // Control: the places are the degenerate ones. At the vertex the edge's logarithm is
    // infinite (its term is dropped, or V would be NaN); on the edge the point is within rounding
    // of the edge's line (r₀ ~ ε|p|, so L = ln(2ℓ/gap) is past 50); the point on the face is within
    // rounding of its plane.
    let solid = &surface_bodies()[1];
    let places = surface_places(solid);
    let f = solid.faces[3];
    let e = solid.edges[f.edges[0] as usize];
    let log_at = |p: Vec3| {
        let (ra, rb) = (solid.vertices[e.ends[0] as usize] - p, solid.vertices[e.ends[1] as usize] - p);
        edge_log(ra, rb, ra.length(), rb.length(), e.direction, e.length)
    };
    let (on_edge, at_vertex) = (log_at(places[1].1), log_at(places[2].1));
    assert!(on_edge > 50.0, "the edge's logarithm is {on_edge}");
    assert!(at_vertex.is_infinite() && (0.0 * at_vertex).is_nan(), "{at_vertex}");
    let h = f.normal.dot(solid.vertices[f.corners[0] as usize] - places[0].1);
    assert!(h.abs() <= PLANE_ROUNDING * (places[0].1.length() + solid.reach), "{h:e}");
}

cav_validate::case! {
    test: validation_polyhedral_on_its_surface,
    name: "polyhedral.on_its_surface",
    subsystem: "polyhedral",
    slice: "S5",
    reference: ClosedForm,
    against: "P4: V and g at a point on a face, on an edge, at a vertex and on an edge's extension, against points 1e-9 of the size away on each side, of an aligned box, an icosahedron and a turned box: the worst relative change (finite everywhere)",
    unit: Relative,
    tolerance: 1e-7,
    measure: cav_validate::Measured::Error(surface_bodies().iter().map(surface_continuity).fold(0.0, f64::max)),
}

// ---------------------------------------------------------------------------------------------
// P5: the solid angles say inside or out.

/// Whether `p` is inside the convex `solid` by its faces' planes, with a margin of `margin`
/// metres: `None` within it.
fn convex_side(solid: &Faceted, p: Vec3, margin: f64) -> Option<bool> {
    let worst = solid
        .faces
        .iter()
        .map(|f| f.normal.dot(p - solid.vertices[f.corners[0] as usize]))
        .fold(f64::NEG_INFINITY, f64::max);
    if worst < -margin {
        Some(true)
    } else if worst > margin {
        Some(false)
    } else {
        None
    }
}

const TORUS: (f64, f64) = (0.7, 0.25);

/// The bodies of P5 and P6: the five Platonic solids, a turned box and a torus (not convex).
fn angle_bodies() -> Vec<(String, Faceted, bool)> {
    let mut out: Vec<(String, Faceted, bool)> = Platonic::ALL
        .iter()
        .map(|s| (format!("{s:?}"), Faceted::from_polyhedron(&s.solid(0.9).moved(ODD)).unwrap(), true))
        .collect();
    out.push(("turned box".into(), turned_box(ODD, BOX_HALF), true));
    let (v, t) = torus_mesh(TORUS.0, TORUS.1, 48, 24);
    let v = v.into_iter().map(|p| p + ODD).collect();
    out.push(("torus".into(), Faceted::indexed(v, &t).unwrap(), false));
    out
}

/// Inside the torus about `ODD` (by the tube's distance), with a margin of a fifth of the tube.
fn torus_side(p: Vec3) -> Option<bool> {
    let d = p - ODD;
    let ring = ((d.x * d.x + d.y * d.y).sqrt() - TORUS.0).hypot(d.z);
    if ring < 0.8 * TORUS.1 {
        Some(true)
    } else if ring > 1.2 * TORUS.1 {
        Some(false)
    } else {
        None
    }
}

/// The worst `|Σω − want|` over inside, outside and on-face points of every body, and how many
/// points of each kind were tried.
fn solid_angle_error(bodies: &[(String, Faceted, bool)]) -> (f64, [usize; 3]) {
    let mut worst = 0f64;
    let mut counts = [0usize; 3];
    for (_, solid, convex) in bodies {
        let s = solid.size();
        for (k, n) in directions(200).into_iter().enumerate() {
            let p = ODD + n * (s * 1.2 * spread(k));
            let side = if *convex { convex_side(solid, p, 1e-3 * s) } else { torus_side(p) };
            let want = match side {
                Some(true) => 4.0 * PI,
                Some(false) => 0.0,
                None => continue,
            };
            counts[usize::from(side == Some(true))] += 1;
            worst = worst.max((solid.solid_angle(p) - want).abs());
        }
        if !*convex {
            // The tube's middle (inside) and the hole and beyond (outside), all around.
            for k in 0..24 {
                let a = 2.0 * PI * (k as f64 + 0.3) / 24.0;
                let round = Vec3::new(a.cos(), a.sin(), 0.0);
                let tube = ODD + round * TORUS.0 + Vec3::new(0.0, 0.0, 0.4 * TORUS.1 * (a * 3.0).sin());
                worst = worst.max((solid.solid_angle(tube) - 4.0 * PI).abs());
                worst = worst.max(solid.solid_angle(ODD + round * (0.3 * TORUS.0)).abs());
                worst = worst.max(solid.solid_angle(ODD + round * (TORUS.0 + 2.0 * TORUS.1)).abs());
                counts[0] += 2;
                counts[1] += 1;
            }
        }
        for (k, f) in solid.faces.iter().enumerate().take(12) {
            let [a, b, c] = f.corners.map(|i| solid.vertices[i as usize]);
            let (u, w) = (0.1 + 0.6 * spread(k), 0.1 + 0.25 * spread(k + 7));
            let p = a * (1.0 - u - w) + b * u + c * w;
            counts[2] += 1;
            worst = worst.max((solid.solid_angle(p) - 2.0 * PI).abs());
        }
    }
    (worst, counts)
}

#[test]
fn the_solid_angles_say_inside_or_out() {
    let bodies = angle_bodies();
    let (worst, counts) = solid_angle_error(&bodies);
    assert!(worst <= 1e-12, "{worst:e}");
    assert!(counts.iter().all(|&c| c >= 50), "too few points of a kind: {counts:?}");
    // Control: one face's winding reversed reads neither 0 outside nor 4π inside.
    let broken: Vec<(String, Faceted, bool)> =
        bodies.iter().map(|(n, s, c)| (n.clone(), flipped(s), *c)).take(1).collect();
    let broken_worst = solid_angle_error(&broken).0;
    assert!(broken_worst > 1e-12, "a flipped face still reads right: {broken_worst:e}");
}

cav_validate::case! {
    test: validation_polyhedral_solid_angles,
    name: "polyhedral.solid_angles",
    subsystem: "polyhedral",
    slice: "S5",
    reference: ClosedForm,
    against: "P5: the sum of the faces' solid angles at points outside (0), inside (4 pi) and on a face (2 pi) of the five Platonic solids, a turned box and a torus mesh: the worst difference in steradians (the report has no solid-angle unit: read radians as steradians)",
    unit: Radians,
    tolerance: 1e-12,
    measure: cav_validate::Measured::Error(solid_angle_error(&angle_bodies()).0),
}

// ---------------------------------------------------------------------------------------------
// P6: the attraction is the gradient.

/// The worst relative difference between `g` and minus a central difference of `V` (step 1e-5
/// of the size), at points outside (1.2 to 3 sizes) and inside, for `solid`.
fn gradient_error(solid: &Faceted, convex: bool) -> f64 {
    let s = solid.size();
    let centre = solid.moments(1.0).centre;
    let mut worst = 0f64;
    for (k, n) in directions(24).into_iter().enumerate() {
        let outside = centre + n * (s * (1.2 + 1.8 * spread(k)));
        let inside = if convex {
            centre + n * (0.15 * s * spread(k + 3))
        } else {
            ODD + Vec3::new(TORUS.0, 0.0, 0.0) + n * 0.1
        };
        for p in [outside, inside] {
            let h = 1e-5 * s;
            let v = |q: Vec3| solid.potential(DENSITY, q);
            let along = |axis: Vec3| -(v(p + axis * h) - v(p - axis * h)) / (2.0 * h);
            let numerical = Vec3::new(along(Vec3::X), along(Vec3::Y), along(Vec3::Z));
            worst = worst.max(relative_vec(solid.attraction(DENSITY, p), numerical));
        }
    }
    worst
}

#[test]
fn the_attraction_is_the_gradient() {
    for (name, solid, convex) in angle_bodies() {
        let worst = gradient_error(&solid, convex);
        assert!(worst <= 1e-7, "{name}: {worst:e}");
        // It points at the mass: from far along +x, towards −x.
        let g = solid.attraction(DENSITY, solid.moments(1.0).centre + Vec3::X * (10.0 * solid.size()));
        assert!(g.x < 0.0 && g.x.abs() > 10.0 * (g.y.abs() + g.z.abs()), "{name}: {g:?}");
    }
    // Control: with one face's normal flipped, the per-face terms no longer sum to a gradient.
    let (_, solid, convex) = &angle_bodies()[4];
    let broken = gradient_error(&flipped(solid), *convex);
    assert!(broken > 1e-7, "a flipped face's attraction is still the gradient: {broken:e}");
}

cav_validate::case! {
    test: validation_polyhedral_attraction_is_the_gradient,
    name: "polyhedral.attraction_is_the_gradient",
    subsystem: "polyhedral",
    slice: "S5",
    reference: ClosedForm,
    against: "P6: g against minus a central difference of V (step 1e-5 of the size) at 48 points outside and inside the five Platonic solids, a turned box and a torus mesh: the worst relative difference",
    unit: Relative,
    tolerance: 1e-7,
    measure: cav_validate::Measured::Error(
        angle_bodies().iter().map(|(_, s, convex)| gradient_error(s, *convex)).fold(0.0, f64::max)
    ),
}

// ---------------------------------------------------------------------------------------------
// P7: a mesh's exact moments.

/// The worst error of `got` against `want`: mass relative, the centre over the size, second
/// moments over `m s²` and third over `m s³`.
fn moments_error(got: &Moments, want: &Moments, size: f64) -> f64 {
    let m = want.mass;
    let mut worst = relative(got.mass, m).max((got.centre - want.centre).length() / size);
    for i in 0..6 {
        worst = worst.max((got.second[i] - want.second[i]).abs() / (m * size * size));
    }
    for i in 0..10 {
        worst = worst.max((got.third[i] - want.third[i]).abs() / (m * size.powi(3)));
    }
    worst
}

/// The closed-form moments of the Platonic solid of edge `edge` centred at `centre`: volume by
/// the textbook factor, an isotropic second moment `I/2` (`I` the textbook moment of inertia),
/// third moments zero except the tetrahedron's `xyz = m k³/15` (`k = edge/2√2`, its vertices at
/// `k(±1, ±1, ±1)` with an even count of minuses: for a simplex with `Σ v_i = 0`,
/// `∫xyz = V/60 Σ x_i y_i z_i`).
fn platonic_moments(solid: Platonic, edge: f64, centre: Vec3) -> Moments {
    let mass = DENSITY * solid.volume_factor() * edge.powi(3);
    let s = 0.5 * solid.inertia_factor() * mass * edge * edge;
    let mut third = [0.0; 10];
    if solid == Platonic::Tetrahedron {
        let k = edge / (2.0 * std::f64::consts::SQRT_2);
        third[9] = mass * k.powi(3) / 15.0;
    }
    Moments { mass, centre, second: [s, s, s, 0.0, 0.0, 0.0], third }
}

/// The closed-form moments of the turned box: `R diag(m h²/3) Rᵀ`, third moments zero.
fn turned_box_moments(centre: Vec3, half: Vec3) -> Moments {
    let mass = DENSITY * 8.0 * half.x * half.y * half.z;
    let (axis, angle) = turn();
    let columns = [Vec3::X, Vec3::Y, Vec3::Z].map(|e| turned(e, axis, angle));
    let own = [half.x, half.y, half.z].map(|h| mass * h * h / 3.0);
    let at = |a: usize, b: usize| -> f64 {
        (0..3).map(|k| columns[k].to_array()[a] * own[k] * columns[k].to_array()[b]).sum()
    };
    Moments {
        mass,
        centre,
        second: [at(0, 0), at(1, 1), at(2, 2), at(0, 1), at(0, 2), at(1, 2)],
        third: [0.0; 10],
    }
}

/// The worst error over the five Platonic solids (edge 0.9 m, at a non-dyadic centre) and the
/// turned box, against the closed forms and against `Polyhedron`'s own integrals.
fn mesh_moments_error() -> f64 {
    let mut worst = 0f64;
    let edge = 0.9;
    for solid in Platonic::ALL {
        let polyhedron = solid.solid(edge).moved(ODD);
        let faceted = Faceted::from_polyhedron(&polyhedron).unwrap();
        let got = faceted.moments(DENSITY);
        worst = worst.max(moments_error(&got, &platonic_moments(solid, edge, ODD), faceted.size()));
        // And Polyhedron's integrals (volume, centroid, second moments).
        let s = polyhedron.second_moments().map(|x| DENSITY * x);
        let theirs = Moments {
            mass: DENSITY * polyhedron.volume(),
            centre: polyhedron.centroid(),
            second: s,
            third: got.third,
        };
        worst = worst.max(moments_error(&got, &theirs, faceted.size()));
    }
    let solid = turned_box(ODD, BOX_HALF);
    worst.max(moments_error(&solid.moments(DENSITY), &turned_box_moments(ODD, BOX_HALF), solid.size()))
}

cav_validate::case! {
    test: validation_a_mesh_s_moments_are_exact,
    name: "polyhedral.mesh_moments",
    subsystem: "polyhedral",
    slice: "S5",
    reference: ClosedForm,
    against: "volume, centroid, second and third central moments by the divergence theorem over the triangles of the five Platonic solids (edge 0.9 m, non-dyadic centre) and a turned 0.62×0.94×0.46 m box, against the textbook closed forms and Polyhedron's integrals: the worst error (mass relative, centre over the size, moments over m·s² and m·s³)",
    unit: Relative,
    tolerance: 1e-13,
    measure: cav_validate::Measured::Error(mesh_moments_error()),
}

#[test]
fn the_moments_see_a_tetrahedron_s_third_moment() {
    // Control: the tetrahedron's third moment is not zero, and zeroing it is seen.
    let edge = 0.9;
    let faceted = Faceted::platonic(Platonic::Tetrahedron, edge).unwrap();
    let mut got = faceted.moments(DENSITY);
    let want = platonic_moments(Platonic::Tetrahedron, edge, Vec3::ZERO);
    assert!(moments_error(&got, &want, faceted.size()) <= 1e-13);
    got.third = [0.0; 10];
    assert!(moments_error(&got, &want, faceted.size()) > 1e-3);
}

#[test]
fn the_moments_are_in_the_tree_s_index_order() {
    // A right tetrahedron, its every second and third moment different: the sums over its four
    // faces against the simplex's own closed form (about its centroid Σq = 0, so
    // ∫x_a x_b = V/20 Σ q_a q_b and ∫x_a x_b x_c = V/60 Σ q_a q_b q_c), read through
    // `Moments::second_at` and `third_at`, which hold the tree's order.
    let corners = [Vec3::ZERO, Vec3::new(0.7, 0.0, 0.0), Vec3::new(0.0, 0.4, 0.0), Vec3::new(0.0, 0.0, 0.3)];
    let corners = corners.map(|c| c + ODD);
    let solid = Faceted::indexed(corners.to_vec(), &[[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]]).unwrap();
    let m = solid.moments(DENSITY);
    let volume = 0.7 * 0.4 * 0.3 / 6.0;
    let centre = corners.iter().fold(Vec3::ZERO, |s, c| s + *c) * 0.25;
    let q = corners.map(|c| (c - centre).to_array());
    let scale = DENSITY * volume;
    let size = solid.size();
    assert!(relative(m.mass, scale) < 1e-14 && (m.centre - centre).length() < 1e-15);
    let mut seen = Vec::new();
    for a in 0..3 {
        for b in 0..3 {
            let want: f64 = scale / 20.0 * q.iter().map(|p| p[a] * p[b]).sum::<f64>();
            assert!((m.second_at(a, b) - want).abs() <= 1e-13 * scale * size * size, "{a}{b}");
            for c in 0..3 {
                let want: f64 = scale / 60.0 * q.iter().map(|p| p[a] * p[b] * p[c]).sum::<f64>();
                assert!((m.third_at(a, b, c) - want).abs() <= 1e-13 * scale * size.powi(3), "{a}{b}{c}");
                seen.push(want);
            }
        }
    }
    // Control: the ten third moments differ from one another, so a slot out of order is caught.
    let mut distinct: Vec<f64> = m.third.to_vec();
    distinct.sort_by(f64::total_cmp);
    assert!(distinct.windows(2).all(|w| w[1] - w[0] > 1e-6 * scale * size.powi(3)), "{distinct:?}");
    assert_eq!(seen.len(), 27);
}

// ---------------------------------------------------------------------------------------------
// Construction.

#[test]
fn a_faceted_solid_is_refused_when_it_is_not_one() {
    let (v, t) = box_mesh(Vec3::ZERO, Vec3::new(1.0, 2.0, 3.0));
    let good = Faceted::indexed(v.clone(), &t).unwrap();
    assert_eq!(good.faces().len(), 12);
    assert_eq!(good.edges().len(), 18);
    assert!((good.volume() - 6.0).abs() < 1e-14);
    assert!((good.size() - 14f64.sqrt()).abs() < 1e-14);
    // From loose triangles: the same solid, the same bits.
    let loose: Vec<[Vec3; 3]> = t.iter().map(|tri| tri.map(|i| v[i as usize])).collect();
    let again = Faceted::new(&loose).unwrap();
    let p = Vec3::new(0.3, 2.7, -0.4);
    assert_eq!(good.potential(1.0, p).to_bits(), again.potential(1.0, p).to_bits());
    // An open box, a box inside out, a triangle with no area, an index out of range.
    assert!(Faceted::indexed(v.clone(), &t[..11]).is_err());
    let inside_out: Vec<[u32; 3]> = t.iter().map(|tri| [tri[0], tri[2], tri[1]]).collect();
    assert!(Faceted::indexed(v.clone(), &inside_out).unwrap_err().0.contains("inside out"));
    let mut bad = t.clone();
    bad[0] = [0, 0, 1];
    assert!(Faceted::indexed(v.clone(), &bad).is_err());
    bad[0] = [0, 1, 99];
    assert!(Faceted::indexed(v.clone(), &bad).is_err());
    let mut far = v;
    far[0] = Vec3::new(f64::NAN, 0.0, 0.0);
    assert!(Faceted::indexed(far, &t).is_err());
    assert!(Faceted::platonic(Platonic::Cube, 0.0).is_err());
    // A mesh and the Platonic solids convert.
    let (tv, tt) = torus_mesh(0.7, 0.25, 12, 8);
    let mesh = Mesh::new(tv, tt).unwrap();
    assert!(Faceted::from_mesh(&mesh).is_ok());
    for s in Platonic::ALL {
        let f = Faceted::platonic(s, 0.5).unwrap();
        assert!((f.volume() / (s.volume_factor() * 0.125) - 1.0).abs() < 1e-13, "{s:?}");
    }
}

#[test]
fn the_difference_is_the_two_potentials() {
    let solid = Faceted::from_polyhedron(&Platonic::Dodecahedron.solid(0.4).moved(ODD)).unwrap();
    let (across, below, above) = ([0.9, -0.4], 0.3, 1.7);
    let d = solid.difference(DENSITY, across, below, above, below - above);
    let want = solid.potential(DENSITY, Vec3::new(0.9, -0.4, 1.7))
        - solid.potential(DENSITY, Vec3::new(0.9, -0.4, 0.3));
    assert_eq!(d.to_bits(), want.to_bits());
    assert_eq!(solid.difference(DENSITY, across, 1.0, 1.0, 0.0), 0.0);
    // Control: the order of the points matters.
    assert!(solid.difference(DENSITY, across, above, below, above - below) == -d && d != 0.0);
}

// ---------------------------------------------------------------------------------------------
// Measurements (release, one thread): `cargo test -p cav-core --release --lib polyhedral --
// --ignored --nocapture --test-threads=1`.

#[test]
#[ignore = "a measurement: ns per face per point of the potential and of the difference"]
fn timing_ns_per_face() {
    let (v, t) = torus_mesh(0.7, 0.25, 64, 32);
    let solid = Faceted::indexed(v, &t).unwrap();
    let faces = solid.faces().len() as f64;
    let points: Vec<Vec3> = directions(400).into_iter().map(|n| n * 2.5).collect();
    let mut sink = 0.0;
    for _ in 0..2 {
        let start = std::time::Instant::now();
        for p in &points {
            sink += solid.potential(DENSITY, *p);
        }
        let potential = start.elapsed().as_secs_f64() * 1e9 / (faces * points.len() as f64);
        let start = std::time::Instant::now();
        for p in &points {
            sink += solid.difference(DENSITY, [p.x, p.y], p.z - 1.0, p.z, -1.0);
        }
        let difference = start.elapsed().as_secs_f64() * 1e9 / (faces * points.len() as f64);
        println!(
            "{faces} faces: potential {potential:.1} ns per face per point, difference {difference:.1} (two points)"
        );
    }
    assert!(sink.is_finite());
}

#[test]
#[ignore = "a measurement: where the closed form's difference V(upper) − V(lower) keeps its digits"]
fn difference_by_distance() {
    // A 1 m cube of 2650 kg/m³ at a non-dyadic centre, against the cube kernel's difference (the
    // series with the monopole differenced without cancellation), gaps of s and 10 s.
    let side = 1.0;
    let cube = Faceted::from_cuboid(&Cuboid { centre: ODD, half: Vec3::new(0.5, 0.5, 0.5) * side }).unwrap();
    let mass = DENSITY * side.powi(3);
    for gap in [1.0, 10.0] {
        for ratio in [3.0, 10.0, 30.0, 100.0, 300.0, 1e3, 3e3, 1e4] {
            let mut worst = 0f64;
            for n in directions(16) {
                let p = ODD + n * (ratio * cube.size());
                let (below, above) = (p.z - gap, p.z);
                let got = cube.difference(DENSITY, [p.x, p.y], below, above, -gap);
                let want = cube_difference(
                    mass,
                    side,
                    [p.x - ODD.x, p.y - ODD.y],
                    below - ODD.z,
                    above - ODD.z,
                    -gap,
                );
                worst = worst.max(relative(got, want));
            }
            println!("gap {gap} s, R/s {ratio:6.0e}: worst relative error of the difference {worst:.1e}");
        }
    }
}

#[test]
#[ignore = "a diagnostic: each row's measured number, for the slice's notes"]
fn the_rows_measured() {
    let (v, g) = against_the_prism(&the_box());
    println!("P1: V {v:.1e}, g {g:.1e}");
    let near: Vec<(f64, f64)> = [10.0, 30.0, 100.0, 300.0, 1000.0].into_iter().map(far_field_error).collect();
    let (k4, c4) = far_field_error(1e4);
    println!(
        "P2: R/s 10..1e3 worst {:.1e}; 1e4 {k4:.1e}, triple product {c4:.1e}",
        near.iter().map(|x| x.0).fold(0.0, f64::max)
    );
    let rows: Vec<_> = NEAR_DISTANCES.into_iter().map(near_edge).collect();
    let worst = rows.iter().map(|r| r.0.max(r.2).max(r.3)).fold(0.0, f64::max);
    println!("P3: worst {worst:.1e}; direct at 1e-8 m {:.1e}", rows[4].1);
    let p4 = surface_bodies().iter().map(surface_continuity).fold(0.0, f64::max);
    println!("P4: worst change {p4:.1e}");
    let (p5, counts) = solid_angle_error(&angle_bodies());
    println!("P5: worst {p5:.1e} over {counts:?} (outside, inside, on a face)");
    let p6 = angle_bodies().iter().map(|(_, s, c)| gradient_error(s, *c)).fold(0.0, f64::max);
    println!("P6: worst {p6:.1e}");
    println!("P7: worst {:.1e}", mesh_moments_error());
}
