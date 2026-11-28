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

/// The calibration set's bodies (`oracle/fixtures/paper/polyhedral_calibration/`, made by the
/// paper's `scripts/calibration_fixtures.py`): not the six, so that the rounding law's constant is
/// not fitted to the bodies it judges (`docs/slices/S5.md` §7.19).
const CALIBRATION: [&str; 9] = [
    "tetrahedron",
    "octahedron",
    "cube",
    "box",
    "plate_10",
    "plate_100",
    "rod_20",
    "icosphere_320",
    "l_prism",
];

/// One evaluation measured against the 40-digit reference: the point's `R/s` (distance from the
/// body's centroid over `Faceted::size`) and the relative errors of `U` and `g`.
#[derive(Clone, Copy, Debug)]
struct Measured {
    ratio: f64,
    u: f64,
    g: f64,
}

impl Measured {
    /// The larger error over `ε(R/s)²`: what the rounding law holds below its constant.
    fn law(self) -> f64 {
        self.u.max(self.g) / (f64::EPSILON * self.ratio * self.ratio)
    }
}

/// `solid`'s `R/s` at `point`.
fn ratio(solid: &Faceted, point: Vec3) -> f64 {
    (point - solid.moments(1.0).centre).length() / solid.size()
}

/// The calibration set measured: every body, every point.
fn calibration() -> Vec<(&'static str, Measured)> {
    let mut out = Vec::new();
    for body in CALIBRATION {
        let read = |name: &str| fixture(&format!("paper/polyhedral_calibration/{body}/{name}.npy"));
        let flat = read("triangles");
        let triangles: Vec<[Vec3; 3]> =
            flat.chunks(9).map(|t| [0, 3, 6].map(|k| Vec3::new(t[k], t[k + 1], t[k + 2]))).collect();
        let solid = Faceted::new(&triangles).unwrap_or_else(|e| panic!("{body}: {e:?}"));
        let (uh, ul, gh, gl) = (read("ref_U_hi"), read("ref_U_lo"), read("ref_g_hi"), read("ref_g_lo"));
        for (k, c) in read("points").chunks(3).enumerate() {
            let p = Vec3::new(c[0], c[1], c[2]);
            let field = solid.field(1.0, p);
            let g = [field.attraction.x, field.attraction.y, field.attraction.z];
            let reference = [0, 1, 2].map(|i| (gh[3 * k + i], gl[3 * k + i]));
            out.push((
                body,
                Measured {
                    ratio: ratio(&solid, p),
                    u: relative(-field.potential, (uh[k], ul[k])),
                    g: relative_vec(g, reference),
                },
            ));
        }
    }
    out
}

/// The six bodies measured at every point within 100 sizes, by `values` (the engine's, or ESA's
/// at its 18 points).
fn the_six(esa: bool) -> Vec<(&'static str, f64, Measured)> {
    let mut out = Vec::new();
    for body in WITH_ESA {
        let solid = solid(body);
        let errors: Vec<(f64, f64)> = if esa {
            esa_errors(body)
        } else {
            cavendish_errors(body).into_iter().map(|(_, u, g)| (u, g)).collect()
        };
        for (p, (u, g)) in points().into_iter().zip(errors) {
            let ratio = ratio(&solid, p);
            if ratio <= 100.0 {
                out.push((body, p.length(), Measured { ratio, u, g }));
            }
        }
    }
    out
}

/// The rounding law's constant `c` (`docs/slices/S5.md` §7.19): the relative error of `U` and of
/// `g` is at most `c·ε·(R/s)²`. Fixed on the calibration set, not the six, before the six were
/// measured: the largest `e/(ε(R/s)²)` there, rounded up to two significant figures. Measured
/// 1927.5 (the 20:1 rod at `R/s` = 100; the 100:1 plate 1049 at 3; the compact bodies 7 to 44).
const ROUNDING_LAW: f64 = 2000.0;

/// Prints the calibration set's largest `e/(ε(R/s)²)` and where it is: how [`ROUNDING_LAW`] was
/// fixed. `cargo test --manifest-path oracle/Cargo.toml --test polyhedral -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement: run by hand"]
fn the_rounding_law_s_constant_measured() {
    let measured = calibration();
    for body in CALIBRATION {
        let (worst, at) = measured
            .iter()
            .filter(|(b, _)| *b == body)
            .map(|(_, m)| (m.law(), m.ratio))
            .fold((0.0, 0.0), |a, b| if b.0 > a.0 { b } else { a });
        println!("{body:14} worst e/(eps (R/s)^2) {worst:.3} at R/s {at}");
    }
    let worst = measured.iter().map(|(_, m)| m.law()).fold(0.0, f64::max);
    println!("calibration: c >= {worst:.4}");
    // P8 and its control, against the constant fixed above (printed after it was fixed).
    for (who, esa) in [("Cavendish", false), ("ESA", true)] {
        for body in WITH_ESA {
            let (law, r, ratio) = the_six(esa)
                .into_iter()
                .filter(|(b, _, _)| *b == body)
                .map(|(_, r, m)| (m.law(), r, m.ratio))
                .fold((0.0, 0.0, 0.0), |a, b| if b.0 > a.0 { b } else { a });
            println!("{who:9} {body:13} worst e/(eps (R/s)^2) {law:.3} at {r} m (R/s {ratio:.2})");
        }
    }
}

#[test]
fn the_rounding_law_holds_on_the_calibration_set() {
    let (worst, body, at) = calibration()
        .into_iter()
        .map(|(b, m)| (m.law(), b, m.ratio))
        .fold((0.0, "", 0.0), |a, b| if b.0 > a.0 { b } else { a });
    assert!(worst <= ROUNDING_LAW, "{body} at R/s {at}: {worst:.3} eps (R/s)^2 (c = {ROUNDING_LAW})");
}

/// P8 as the owner restated it (28 Nov 2026, `docs/slices/S5.md` §7.19): on the six bodies, every
/// point within 100 sizes, the relative error of `U` and of `g` at most `c·ε·(R/s)²`, `c` the
/// calibration set's ([`ROUNDING_LAW`]). As §3 first wrote it (≤ 1e-12) it measured 1.6e-10.
cav_validate::case! {
    test: polyhedral_agrees_with_the_40_digit_reference,
    name: "polyhedral.forty_digit_reference",
    subsystem: "polyhedral",
    slice: "S5",
    reference: Recording,
    against: "P8 restated by the owner's ruling of 28 Nov 2026 (docs/slices/S5.md 7.19; as first written, 1e-12 relative to 100 sizes, it measured 1.6e-10): Faceted::field against a 40-digit evaluation of the same face form on the same f64 vertices (docs/papers/polyhedral-gravity's mp_reference, oracle/fixtures/paper/polyhedral), the six bodies, every point within 100 sizes, U and g: the largest relative error over the rounding law's allowance c eps (R/s)^2, c = 2000 fixed first on nine other bodies (oracle/fixtures/paper/polyhedral_calibration). Control: ESA's values exceed it",
    unit: Relative,
    tolerance: 1.0,
    measure: cav_validate::Measured::Error(
        the_six(false).into_iter().map(|(_, _, m)| m.law() / ROUNDING_LAW).fold(0.0, f64::max)
    ),
}

#[test]
fn esa_s_values_break_the_rounding_law() {
    // Control of P8: ESA's values, the eps (R/s)^3 class, from the same fixture, exceed it.
    let worst = the_six(true).into_iter().map(|(_, _, m)| m.law()).fold(0.0, f64::max);
    assert!(worst > ROUNDING_LAW, "ESA's values are within {worst:.3} eps (R/s)^2 (c = {ROUNDING_LAW})");
}
