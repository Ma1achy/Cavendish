//! The polyhedral closed form (`cav_core::polyhedral::Faceted`) against a 40-digit evaluation of
//! the same formula on the same f64 vertices, and against ESA's tool as recorded
//! (`docs/papers/polyhedral-gravity`, `data/esa_fixtures.json` and `data/mp_reference.json`; the
//! fixtures under `fixtures/paper/polyhedral/` are those two files converted, with the 40-digit
//! values `mp_reference.py` evaluated, by `scripts/oracle_fixtures.py`). `docs/slices/S5.md` §3
//! row P8 and the section that records it.
//!
//! Units: `U = ∫dV/r` (m²) and `g = ∇U` (m) at density 1 with no G, ESA's; the engine's `Faceted`
//! gives `potential = −U` and `attraction = g` at density 1.

use cav_core::Vec3;
use cav_core::polyhedral::Faceted;
use oracle::fixture;

/// The bodies with ESA's values, in `esa_fixtures.json`'s order.
const WITH_ESA: [&str; 6] = ["cube_aligned", "cube", "icosahedron", "plate", "sphere", "stellated"];
/// Every body of the fixture: the six and the research benchmark's axis-aligned plate.
const BODIES: [&str; 7] =
    ["cube_aligned", "cube", "icosahedron", "plate", "sphere", "stellated", "plate_aligned"];
/// The points' distances from the origin, metres; three directions each, distance-major.
const DISTANCES: [f64; 8] = [1.0, 1.5, 3.0, 10.0, 30.0, 100.0, 300.0, 1000.0];
/// How many of the points ESA was run at (the first six distances).
const ESA_POINTS: usize = 18;

fn points() -> Vec<Vec3> {
    fixture("paper/polyhedral/points.npy").chunks(3).map(|c| Vec3::new(c[0], c[1], c[2])).collect()
}

fn solid(body: &str) -> Faceted {
    let flat = fixture(&format!("paper/polyhedral/{body}/triangles.npy"));
    let triangles: Vec<[Vec3; 3]> =
        flat.chunks(9).map(|t| [0, 3, 6].map(|k| Vec3::new(t[k], t[k + 1], t[k + 2]))).collect();
    Faceted::new(&triangles).unwrap_or_else(|e| panic!("{body}: {e:?}"))
}

/// The 40-digit `U` and `g` at every point, as double-doubles `(hi, lo)`.
struct Reference {
    u: Vec<(f64, f64)>,
    g: Vec<[(f64, f64); 3]>,
}

fn reference(body: &str) -> Reference {
    let read = |name: &str| fixture(&format!("paper/polyhedral/{body}/{name}.npy"));
    let (uh, ul, gh, gl) = (read("ref_U_hi"), read("ref_U_lo"), read("ref_g_hi"), read("ref_g_lo"));
    Reference {
        u: uh.into_iter().zip(ul).collect(),
        g: gh.chunks(3).zip(gl.chunks(3)).map(|(h, l)| [0, 1, 2].map(|i| (h[i], l[i]))).collect(),
    }
}

/// `|x − ref| / |ref|` with the reference a double-double: `x − hi` is exact when the two are
/// close, so the rounding of the reference to f64 does not enter.
fn relative(x: f64, (hi, lo): (f64, f64)) -> f64 {
    ((x - hi) - lo).abs() / (hi + lo).abs()
}

/// `|g − ref| / |ref|`, the vector's norms, as `mp_reference.py` takes them.
fn relative_vec(g: [f64; 3], reference: [(f64, f64); 3]) -> f64 {
    let diff: f64 = (0..3).map(|i| ((g[i] - reference[i].0) - reference[i].1).powi(2)).sum();
    let norm: f64 = reference.iter().map(|(h, l)| (h + l).powi(2)).sum();
    (diff / norm).sqrt()
}

/// The engine's relative errors of `U` and `g` at every point of the fixture, `(R, U, g)`.
fn cavendish_errors(body: &str) -> Vec<(f64, f64, f64)> {
    let (solid, reference) = (solid(body), reference(body));
    points()
        .into_iter()
        .enumerate()
        .map(|(k, p)| {
            let field = solid.field(1.0, p);
            let g = [field.attraction.x, field.attraction.y, field.attraction.z];
            (p.length(), relative(-field.potential, reference.u[k]), relative_vec(g, reference.g[k]))
        })
        .collect()
}

/// ESA's relative errors of `U` and `g` at its 18 points, from its values in the fixture.
fn esa_errors(body: &str) -> Vec<(f64, f64)> {
    let reference = reference(body);
    let u = fixture(&format!("paper/polyhedral/{body}/esa_U.npy"));
    let g = fixture(&format!("paper/polyhedral/{body}/esa_g.npy"));
    (0..ESA_POINTS)
        .map(|k| {
            (
                relative(u[k], reference.u[k]),
                relative_vec([g[3 * k], g[3 * k + 1], g[3 * k + 2]], reference.g[k]),
            )
        })
        .collect()
}

/// The worst of the three directions at `distance`, of `errors` given per point.
fn worst_at(distance: f64, errors: impl Iterator<Item = (f64, f64)>) -> f64 {
    errors.filter(|(r, _)| (r / distance - 1.0).abs() < 1e-9).fold(0.0, |m, (_, e)| m.max(e))
}

/// The radius of each point, in the fixture's order.
fn radii() -> Vec<f64> {
    points().iter().map(|p| p.length()).collect()
}

#[test]
fn the_fixture_is_the_recorded_reference() {
    // ESA's errors against the fixture's 40-digit values are the ones mp_reference.json records,
    // so the values are the reference that file was made from.
    let mut worst = 0f64;
    for body in WITH_ESA {
        let recorded_u = fixture(&format!("paper/polyhedral/{body}/recorded_esa_U.npy"));
        let recorded_g = fixture(&format!("paper/polyhedral/{body}/recorded_esa_g.npy"));
        let ours = fixture(&format!("paper/polyhedral/{body}/recorded_ours_g.npy"));
        let mut control = 0f64;
        for (k, (u, g)) in esa_errors(body).into_iter().enumerate() {
            worst = worst.max((u / recorded_u[k] - 1.0).abs()).max((g / recorded_g[k] - 1.0).abs());
            control = control.max((g / ours[k] - 1.0).abs());
        }
        // Control: the same comparison against another code's recorded errors fails.
        assert!(control > 0.1, "{body}: the check cannot tell ESA's errors from the Python form's");
    }
    assert!(worst < 1e-6, "ESA's errors differ from the recorded ones by {worst:e} (relative)");
}

/// The least-squares-free fall-off class: `log₁₀` of the worst-direction error of `g` at 100 m
/// over that at 10 m, the largest over `bodies`.
fn decade_slope(errors: &dyn Fn(&str) -> Vec<(f64, f64)>, bodies: &[&str], pick: fn(f64, f64) -> f64) -> f64 {
    bodies
        .iter()
        .map(|body| {
            let e = errors(body);
            let at = |d| worst_at(d, e.iter().copied());
            (at(100.0) / at(10.0)).log10()
        })
        .reduce(pick)
        .expect("at least one body")
}

fn cavendish_g(body: &str) -> Vec<(f64, f64)> {
    cavendish_errors(body).into_iter().map(|(r, _, g)| (r, g)).collect()
}

fn esa_g(body: &str) -> Vec<(f64, f64)> {
    radii().into_iter().zip(esa_errors(body)).map(|(r, (_, g))| (r, g)).collect()
}

cav_validate::case! {
    test: validation_polyhedral_forty_digit_class,
    name: "polyhedral.forty_digit_class",
    subsystem: "polyhedral",
    slice: "S5",
    reference: Recording,
    against: "Faceted::field against a 40-digit evaluation of the same face form on the same f64 vertices (docs/papers/polyhedral-gravity, data/mp_reference.json's reference, oracle/fixtures/paper/polyhedral), six bodies of about 1 m: log10 of the worst-direction error of g at 100 m over that at 10 m, the largest over the bodies. The eps (R/s)^2 rounding class gives 2, ESA's eps (R/s)^3 gives 3; the tolerance is the middle",
    unit: Slope,
    tolerance: 2.5,
    measure: cav_validate::Measured::Error(decade_slope(&cavendish_g, &WITH_ESA, f64::max)),
}

#[test]
fn esa_s_rounding_grows_a_power_faster() {
    // Control of the class case: ESA's own values, from the same fixture, fail it on every body.
    let least = decade_slope(&esa_g, &WITH_ESA, f64::min);
    assert!(least > 2.5, "ESA's error grows by only 10^{least:.2} a decade on some body");
}

/// The worst relative error of `U` and of `g` over the points within `ratio` sizes of each body
/// (`Faceted::size`: twice its farthest vertex from the centroid), over the six bodies with ESA's
/// values: the measurement of P8 (`docs/slices/S5.md` §3).
fn worst_within(ratio: f64) -> (f64, &'static str, f64) {
    let mut worst = (0.0, "", 0.0);
    for body in WITH_ESA {
        let size = solid(body).size();
        for (r, u, g) in cavendish_errors(body) {
            if r <= ratio * size && u.max(g) > worst.0 {
                worst = (u.max(g), body, r);
            }
        }
    }
    worst
}

/// For the paper (`make.py forty`): every body's errors at every point, written to
/// `docs/papers/polyhedral-gravity/data/cavendish/forty-digits.json`, and P8's measurement printed.
#[test]
#[ignore = "writes the paper's data: cargo test --manifest-path oracle/Cargo.toml --test polyhedral -- --ignored"]
fn forty_digits_for_the_paper() {
    let mut json = String::from(
        "{\n \"reference\": \"40 digits (oracle/fixtures/paper/polyhedral)\",\n \"code\": \"cav_core::polyhedral::Faceted::field\",\n \"bodies\": {\n",
    );
    let list = |v: &[f64]| v.iter().map(|x| format!("{x:e}")).collect::<Vec<_>>().join(", ");
    for (i, body) in BODIES.iter().enumerate() {
        let e = cavendish_errors(body);
        let (r, u, g): (Vec<f64>, Vec<f64>, Vec<f64>) =
            e.iter().fold((vec![], vec![], vec![]), |(mut a, mut b, mut c), (x, y, z)| {
                a.push(*x);
                b.push(*y);
                c.push(*z);
                (a, b, c)
            });
        let faces = solid(body).faces().len();
        json += &format!(
            "  \"{body}\": {{\"triangles\": {faces}, \"size\": {:e}, \"R\": [{}], \"U\": [{}], \"g\": [{}]}}{}\n",
            solid(body).size(),
            list(&r),
            list(&u),
            list(&g),
            if i + 1 < BODIES.len() { "," } else { "" }
        );
        for d in DISTANCES {
            println!(
                "{body:13} R {d:6} m: U {:.1e}, g {:.1e}",
                worst_at(d, e.iter().map(|(r, u, _)| (*r, *u))),
                worst_at(d, e.iter().map(|(r, _, g)| (*r, *g)))
            );
        }
    }
    json += " }\n}\n";
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../docs/papers/polyhedral-gravity/data/cavendish");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("forty-digits.json"), json).unwrap();
    for ratio in [3.0, 10.0, 30.0, 100.0] {
        let (e, body, r) = worst_within(ratio);
        println!("P8: worst within {ratio} sizes: {e:.2e} ({body}, R = {r} m)");
    }
    println!(
        "decade slope: Cavendish {:.3}, ESA (least) {:.3}",
        decade_slope(&cavendish_g, &WITH_ESA, f64::max),
        decade_slope(&esa_g, &WITH_ESA, f64::min)
    );
}

/// P8 as `docs/slices/S5.md` §3 writes it: ≤ 1e-12 relative at `R/s ≤ 100` against the 40-digit
/// reference, `U` and `g`, every point of the six bodies. TASK-S5-28: it fails (1.6e-10 on the
/// turned plate at 100 m, the `ε(R/s)²` class; §7.16), and its PR is left open for the owner's ruling.
#[test]
fn polyhedral_agrees_with_the_40_digit_reference() {
    let (worst, body, r) = worst_within(100.0);
    assert!(worst <= 1e-12, "P8: {worst:e} relative on the {body} at {r} m (criterion 1e-12 within 100 sizes)");
}
