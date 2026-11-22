//! What the engine's exact Lomb–Scargle costs, one thread: why a live cadence preview cannot use it.
use std::time::Instant;
fn main() {
    for n in [10_000usize, 100_000] {
        let t: Vec<f64> = (0..n).map(|i| i as f64 * 5.0 + ((i * 7919) % 1000) as f64 * 1e-4).collect();
        let y: Vec<f64> = t.iter().map(|t| (0.05 * std::f64::consts::TAU * t).cos()).collect();
        let f: Vec<f64> = (1..=1000).map(|k| f64::from(k) * 1e-4).collect();
        let s = Instant::now();
        let p = cav_core::spectrum::lomb_scargle(&t, &y, &f);
        println!(
            "exact Lomb-Scargle, N={n}, M=1000 frequencies, 1 thread: {:.0} ms (checksum {:.3e})",
            s.elapsed().as_secs_f64() * 1e3,
            p.iter().sum::<f64>()
        );
    }
}
