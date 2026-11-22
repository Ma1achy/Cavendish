use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12; self.0 ^= self.0 << 25; self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn unit(&mut self) -> f64 { (self.next() >> 11) as f64 / (1u64 << 53) as f64 }
    fn finite_bits(&mut self) -> f64 { loop { let x = f64::from_bits(self.next()); if x.is_finite() { return x; } } }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct One { x: f64 }

fn samples(n: usize) -> Vec<(&'static str, Vec<f64>)> {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    let all: Vec<f64> = (0..n).map(|_| r.finite_bits()).collect();
    let unit: Vec<f64> = (0..n).map(|_| r.unit()).collect();
    // what a person types: up to 6 significant digits, then a power of ten
    let typed: Vec<f64> = (0..n).map(|_| {
        let m = (r.next() % 999_999 + 1) as f64; let e = (r.next() % 60) as i32 - 30;
        format!("{m}e{e}").parse::<f64>().unwrap()
    }).collect();
    // physics-sized: the scene's scales, 1e-27..1e6
    let phys: Vec<f64> = (0..n).map(|_| 10f64.powf(r.unit() * 33.0 - 27.0) * if r.next() & 1 == 0 { 1.0 } else { -1.0 }).collect();
    let subn: Vec<f64> = (0..n).map(|_| f64::from_bits(r.next() & 0x000F_FFFF_FFFF_FFFF)).collect();
    vec![("random bit patterns (all exponents)", all), ("uniform [0,1)", unit), ("typed decimals (6 digits, e-30..e+30)", typed), ("scene scales (1e-27..1e6, ±)", phys), ("subnormals", subn)]
}

fn json(n: usize) {
    println!("serde_json float_roundtrip = {}", cfg!(feature = "fr"));
    for (name, v) in samples(n) {
        let (mut w_bad, mut v_bad) = (0, 0);
        for &x in &v {
            let s = serde_json::to_string(&One { x }).unwrap();
            if serde_json::from_str::<One>(&s).unwrap().x.to_bits() != x.to_bits() { w_bad += 1; }
            let val = serde_json::to_value(One { x }).unwrap();
            let s2 = serde_json::to_string(&val).unwrap();
            let back: serde_json::Value = serde_json::from_str(&s2).unwrap();
            if serde_json::from_value::<One>(back).unwrap().x.to_bits() != x.to_bits() { v_bad += 1; }
        }
        println!("  {name:<42} struct: {w_bad}/{n} differ; via Value: {v_bad}/{n} differ");
    }
    // text -> f64 against the standard library's correctly rounded parser
    let mut r = Rng(7);
    let mut bad = 0; let mut ex = String::new();
    let m = n;
    for _ in 0..m {
        // 17-significant-digit decimals, as written by other tools (numpy, Python repr gives shortest; %.17g gives 17)
        let x = r.unit() * 10f64.powi((r.next() % 40) as i32 - 20);
        let s = format!("{x:.16e}");
        let want: f64 = s.parse().unwrap();
        let got: f64 = serde_json::from_str(&s).unwrap();
        if got.to_bits() != want.to_bits() { bad += 1; if ex.is_empty() { ex = format!("{s}: got {:e} want {:e}", got, want); } }
    }
    println!("  17-digit text from elsewhere vs std parse: {bad}/{m} differ {ex}");
    // specials
    println!("  specials:");
    for (name, x) in [("NaN", f64::NAN), ("inf", f64::INFINITY), ("-inf", f64::NEG_INFINITY), ("-0.0", -0.0), ("5e-324", 5e-324), ("MAX", f64::MAX), ("2^53+? 1e16", 1e16), ("1.0", 1.0)] {
        let s = serde_json::to_string(&One { x }).unwrap();
        let back = serde_json::from_str::<One>(&s);
        println!("    {name:<10} -> {s:<28} -> {:?}", back.map(|o| format!("bits {:016x}", o.x.to_bits())).map_err(|e| e.to_string()));
    }
    println!("    Number::from_f64(NaN) = {:?}; to_value(NaN) = {:?}", serde_json::Number::from_f64(f64::NAN), serde_json::to_value(f64::NAN).unwrap());
    println!("    Value from 1 (int) set into f64 field: {:?}", serde_json::from_value::<One>(serde_json::json!({"x": 1})));
    println!("    u64 2^53+1 into f64: {:?}", serde_json::from_value::<One>(serde_json::json!({"x": 9007199254740993u64})).map(|o| o.x));
}

fn ron_(n: usize) {
    for (name, v) in samples(n) {
        let mut bad = 0; let mut ex = String::new();
        for &x in &v {
            let s = ron::to_string(&One { x }).unwrap();
            let b: One = ron::from_str(&s).unwrap();
            if b.x.to_bits() != x.to_bits() { bad += 1; if ex.is_empty() { ex = format!("{s}"); } }
        }
        println!("  {name:<42} {bad}/{n} differ {ex}");
    }
    let pretty = ron::ser::PrettyConfig::default();
    println!("  formatting:");
    for x in [1.0f64, 0.1, 1e300, 1e-7, 5e-324, f64::MAX, -0.0, 1.46e-25, 123456789012345680.0, 1e16, 1e15, 0.3, 0.1 + 0.2] {
        let s = ron::to_string(&x).unwrap();
        let p = ron::ser::to_string_pretty(&x, pretty.clone()).unwrap();
        println!("    {:>24e} -> {s:<26} pretty {p:<26} back bits equal: {}", x, ron::from_str::<f64>(&s).map(|b| b.to_bits() == x.to_bits()).unwrap_or(false));
    }
    println!("  specials:");
    for (name, x) in [("NaN", f64::NAN), ("-NaN", -f64::NAN), ("NaN payload 0x7ff8_0000_0000_0001", f64::from_bits(0x7ff8_0000_0000_0001)), ("signalling NaN 0x7ff0_..01", f64::from_bits(0x7ff0_0000_0000_0001)), ("inf", f64::INFINITY), ("-inf", f64::NEG_INFINITY), ("-0.0", -0.0)] {
        let s = ron::to_string(&x).unwrap();
        let b = ron::from_str::<f64>(&s);
        println!("    {name:<36} -> {s:<8} -> {}", match b { Ok(y) => format!("{:016x} (was {:016x})", y.to_bits(), x.to_bits()), Err(e) => format!("error {e}") });
    }
    for text in ["5", "5.", ".5", "1e3", "1E3", "1_000.0", "+1.5", "0x10", "1e400", "-1e400", "1e-400", "inf", "NaN", "0.1f32"] {
        let r = ron::from_str::<f64>(text);
        println!("    parse {text:<9} -> {:?}", r.map(|y| format!("{y:e}")).map_err(|e| e.to_string()));
    }
}

fn npy_enc(shape: &[usize], v: &[f64]) -> Vec<u8> {
    let shp = match shape.len() { 0 => "()".to_string(), 1 => format!("({},)", shape[0]), _ => format!("({})", shape.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ")) };
    let mut h = format!("{{'descr': '<f8', 'fortran_order': False, 'shape': {shp}, }}");
    while (10 + h.len() + 1) % 64 != 0 { h.push(' '); }
    h.push('\n');
    let mut b = b"\x93NUMPY\x01\x00".to_vec();
    b.extend_from_slice(&(h.len() as u16).to_le_bytes());
    b.extend_from_slice(h.as_bytes());
    for x in v { b.extend_from_slice(&x.to_le_bytes()); }
    b
}

fn npy(dir: &str) {
    let mut r = Rng(99);
    let mut v: Vec<f64> = (0..5*7).map(|_| r.finite_bits()).collect();
    v[0] = -0.0; v[1] = f64::NAN; v[2] = f64::from_bits(0x7ff8_dead_beef_0001); v[3] = f64::from_bits(0xfff0_0000_0000_0001); v[4] = 5e-324; v[5] = f64::INFINITY;
    std::fs::write(format!("{dir}/rust_2d.npy"), npy_enc(&[5, 7], &v)).unwrap();
    std::fs::write(format!("{dir}/rust_1d.npy"), npy_enc(&[35], &v)).unwrap();
    std::fs::write(format!("{dir}/rust_0d.npy"), npy_enc(&[], &v[..1])).unwrap();
    std::fs::write(format!("{dir}/rust_empty.npy"), npy_enc(&[0, 3], &[])).unwrap();
    let mut raw = String::new();
    for x in &v { write!(raw, "{:016x}\n", x.to_bits()).unwrap(); }
    std::fs::write(format!("{dir}/bits.txt"), raw).unwrap();
}

fn rayon_() {
    use rayon::prelude::*;
    let mut r = Rng(5);
    let n = 10_000_000;
    // terms of mixed size and sign, as phase contributions are
    let v: Vec<f64> = (0..n).map(|_| (r.unit() - 0.5) * 10f64.powi((r.next() % 9) as i32 - 6)).collect();
    let seq: f64 = v.iter().sum();
    println!("  sequential sum {:016x}", seq.to_bits());
    for threads in [1usize, 2, 3, 4, 8] {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..20 { seen.insert(pool.install(|| v.par_iter().sum::<f64>()).to_bits()); }
        let fixed: Vec<u64> = (0..3).map(|_| pool.install(|| {
            let parts: Vec<f64> = v.par_chunks(4096).map(|c| c.iter().sum::<f64>()).collect();
            parts.iter().sum::<f64>()
        }).to_bits()).collect();
        let tree = pool.install(|| {
            let mut parts: Vec<f64> = v.par_chunks(4096).map(|c| c.iter().sum::<f64>()).collect();
            while parts.len() > 1 { parts = parts.chunks(2).map(|p| p.iter().sum::<f64>()).collect(); }
            parts[0]
        });
        println!("  {threads} threads: par_iter().sum(): {} distinct results in 20 runs; fixed 4096-chunks then ordered sum: {:?}; fixed chunks + fixed pairwise tree: {:016x}", seen.len(), fixed.iter().map(|b| format!("{b:016x}")).collect::<std::collections::BTreeSet<_>>(), tree.to_bits());
    }
}

fn fma() {
    let mut r = Rng(11);
    let n = 1_000_000;
    let (mut differ, mut h_plain, mut h_fma) = (0u64, 0xcbf2_9ce4_8422_2325u64, 0xcbf2_9ce4_8422_2325u64);
    let mut acc = 0.0f64; // a deterministic dependent chain, the way a quadrature accumulates
    for _ in 0..n {
        let (a, b, c) = (r.unit() * 2.0 - 1.0, r.unit() * 2.0 - 1.0, r.unit() * 2.0 - 1.0);
        let p = a * b + c; let f = a.mul_add(b, c);
        if p.to_bits() != f.to_bits() { differ += 1; }
        h_plain = (h_plain ^ p.to_bits()).wrapping_mul(0x0100_0000_01b3);
        h_fma = (h_fma ^ f.to_bits()).wrapping_mul(0x0100_0000_01b3);
        acc = acc * 0.999 + a * b;
    }
    println!("  a*b+c vs mul_add differ in {differ}/{n}; hash of a*b+c: {h_plain:016x}; hash of mul_add: {h_fma:016x}; chain: {:016x}", acc.to_bits());
    println!("  cfg target_feature fma = {}", cfg!(target_feature = "fma"));
}

fn libm_() {
    let mut r = Rng(3);
    let n = 2_000_000;
    let mut out = String::new();
    let (mut s, mut c, mut e, mut l, mut p, mut a, mut t) = (0,0,0,0,0,0,0);
    let (mut s2, mut c2, mut e2) = (0, 0, 0);
    for i in 0..n {
        let x = (r.unit() - 0.5) * 200.0; // phases up to ~100 rad
        let y = r.unit() * 50.0 + 1e-3;
        let ex = (r.unit() - 0.5) * 40.0;
        if x.sin().to_bits() != libm::sin(x).to_bits() { s += 1; }
        if x.cos().to_bits() != libm::cos(x).to_bits() { c += 1; }
        if ex.exp().to_bits() != libm::exp(ex).to_bits() { e += 1; }
        if y.ln().to_bits() != libm::log(y).to_bits() { l += 1; }
        if y.powf(x / 40.0).to_bits() != libm::pow(y, x / 40.0).to_bits() { p += 1; }
        if x.atan2(y).to_bits() != libm::atan2(x, y).to_bits() { a += 1; }
        if x.tan().to_bits() != libm::tan(x).to_bits() { t += 1; }
        if (x.sin_cos().0).to_bits() != x.sin().to_bits() { s2 += 1; }
        if (x.sin_cos().1).to_bits() != x.cos().to_bits() { c2 += 1; }
        // powi vs powf
        if y.powi(3).to_bits() != y.powf(3.0).to_bits() { e2 += 1; }
        if i < 20000 { writeln!(out, "{:016x} {:016x} {:016x} {:016x} {:016x}", x.to_bits(), x.sin().to_bits(), libm::sin(x).to_bits(), ex.exp().to_bits(), libm::exp(ex).to_bits()).unwrap(); }
    }
    println!("  std (system libm) vs `libm` crate 0.2 (musl port), {n} args each: sin {s}, cos {c}, tan {t}, exp {e}, ln {l}, powf {p}, atan2 {a} results differ");
    println!("  std sin_cos vs sin/cos separately: {s2}, {c2} differ;  powi(3) vs powf(3.0): {e2} differ");
    std::fs::write("libm_samples.txt", out).unwrap();
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a[1].as_str() {
        "json" => json(2_000_000), "ron" => ron_(2_000_000), "npy" => npy(&a[2]), "rayon" => rayon_(), "fma" => fma(), "libm" => libm_(), "libm_speed" => libm_speed(),
        _ => {}
    }
}

#[allow(dead_code)]
fn libm_speed() {
    let n = 20_000_000usize;
    let xs: Vec<f64> = (0..n).map(|i| (i as f64) * 1e-5 - 100.0).collect();
    let t = std::time::Instant::now();
    let a: f64 = xs.iter().map(|x| x.sin() + x.cos()).sum();
    let ta = t.elapsed().as_secs_f64();
    let t = std::time::Instant::now();
    let b: f64 = xs.iter().map(|&x| libm::sin(x) + libm::cos(x)).sum();
    let tb = t.elapsed().as_secs_f64();
    println!("  sin+cos over {n} args: std (glibc) {:.0} ms ({:.1} ns/pair); libm crate {:.0} ms ({:.1} ns/pair); sums {:e} {:e}", ta * 1e3, ta / n as f64 * 1e9, tb * 1e3, tb / n as f64 * 1e9, a, b);
}
